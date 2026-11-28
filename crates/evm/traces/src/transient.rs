//! Transient storage (`TLOAD` / `TSTORE`) trace tracking and rendering.

use crate::{
    CallTrace, CallTraceArena, CallTraceNode, CallTraceStep, DecodedTraceStep, TraceMemberOrder,
    TracingInspector,
};
use alloy_primitives::{B256, U256, map::HashMap};
use evm2::{
    EvmTypes,
    interpreter::{InstrStop, Interpreter, op},
};
use serde::{Deserialize, Serialize};
use std::{borrow::Cow, collections::BTreeMap, fmt::Write};

/// Represents the source of a transient storage access (`TLOAD` or `TSTORE`).
#[allow(clippy::upper_case_acronyms)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum TransientStorageChangeReason {
    /// `TLOAD` opcode.
    TLOAD,
    /// `TSTORE` opcode.
    TSTORE,
}

/// Represents a transient storage access recorded at a `TLOAD` or `TSTORE` trace step.
///
/// For a `TLOAD`, `value` is the value read from the slot and `had_value` is `None`. For a
/// `TSTORE`, `value` is the value written and `had_value` is the previous value in the slot.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransientStorageChange {
    /// Index of the step in [`CallTrace::steps`].
    pub step_idx: usize,
    /// Key of the transient storage slot.
    pub key: U256,
    /// Value read by `TLOAD` or written by `TSTORE`.
    pub value: U256,
    /// Value overwritten by `TSTORE`, or `None` for `TLOAD`.
    pub had_value: Option<U256>,
    /// How the transient storage slot was accessed.
    pub reason: TransientStorageChangeReason,
}

#[derive(Clone, Copy, Debug)]
struct PendingTransientStep {
    trace_idx: usize,
    change: TransientStorageChange,
}

/// Sidecar tracer that records `TLOAD` and `TSTORE` accesses alongside a [`TracingInspector`].
///
/// Active frames are kept in lockstep with the tracer through [`Self::enter_node`] and
/// [`Self::exit_node`], and steps are captured via [`Self::start_step`] and [`Self::end_step`].
#[derive(Clone, Debug, Default)]
pub struct TransientStorageTracer {
    active_nodes: Vec<usize>,
    pending: Option<PendingTransientStep>,
    changes: BTreeMap<usize, Vec<TransientStorageChange>>,
}

impl TransientStorageTracer {
    /// Pushes `trace_idx` as the currently executing trace node.
    #[inline]
    pub fn enter_node(&mut self, trace_idx: usize) {
        self.active_nodes.push(trace_idx);
    }

    /// Pops the currently executing trace node and clears any unfinished step.
    #[inline]
    pub fn exit_node(&mut self) {
        self.active_nodes.pop();
        self.pending = None;
    }

    /// Captures operands and prior transient slot state for a `TLOAD` or `TSTORE` step recorded
    /// by `tracer`.
    #[inline]
    pub fn start_step<T: EvmTypes>(
        &mut self,
        interp: &mut Interpreter<'_, '_, T>,
        tracer: &TracingInspector,
    ) {
        self.pending = None;

        let opcode = interp.opcode();
        if !matches!(opcode, op::TLOAD | op::TSTORE) {
            return;
        }
        let config = tracer.config();
        if !config.record_state_diff || !config.record_steps {
            return;
        }
        let Some(&trace_idx) = self.active_nodes.last() else {
            return;
        };
        let Some(node) = tracer.traces().nodes().get(trace_idx) else {
            return;
        };
        let Some(step_idx) = node.trace.steps.len().checked_sub(1) else {
            return;
        };
        let last_step = &node.trace.steps[step_idx];
        if last_step.op.get() != opcode
            || last_step.pc as usize != interp.pc()
            || last_step.gas_cost != 0
            || last_step.status.is_some()
            || self
                .changes
                .get(&trace_idx)
                .and_then(|v| v.last())
                .is_some_and(|c| c.step_idx == step_idx)
        {
            return;
        }

        let (key, value, reason) = match (opcode, interp.stack().as_slice()) {
            (op::TLOAD, [.., key]) => (*key, None, TransientStorageChangeReason::TLOAD),
            (op::TSTORE, [.., value, key]) => {
                (*key, Some(*value), TransientStorageChangeReason::TSTORE)
            }
            _ => return,
        };
        let address = interp.message().destination;
        let current = interp.host().state_mut().tload(&address, &key);
        let change = match value {
            Some(value) => {
                TransientStorageChange { step_idx, key, value, had_value: Some(current), reason }
            }
            None => {
                TransientStorageChange { step_idx, key, value: current, had_value: None, reason }
            }
        };
        self.pending = Some(PendingTransientStep { trace_idx, change });
    }

    /// Commits the pending transient storage access if the instruction succeeded.
    #[inline]
    pub fn end_step<T: EvmTypes>(&mut self, interp: &Interpreter<'_, '_, T>) {
        if let Some(pending) = self.pending.take()
            && interp.result().is_ok()
        {
            self.changes.entry(pending.trace_idx).or_default().push(pending.change);
        }
    }

    /// Discards the pending step when another inspector halts the frame before execution.
    #[inline]
    pub const fn end_step_not_run(&mut self) {
        self.pending = None;
    }

    /// Returns the recorded transient storage accesses keyed by trace node index.
    #[inline]
    pub const fn changes(&self) -> &BTreeMap<usize, Vec<TransientStorageChange>> {
        &self.changes
    }

    /// Consumes the tracer and returns the recorded transient storage accesses keyed by trace
    /// node index.
    #[inline]
    pub fn into_changes(self) -> BTreeMap<usize, Vec<TransientStorageChange>> {
        self.changes
    }
}

/// Filters `transient_storage` to exclude steps removed from `arena` by ignored trace ranges.
pub(crate) fn filter_resolved_changes(
    arena: &CallTraceArena,
    transient_storage: &BTreeMap<usize, Vec<TransientStorageChange>>,
    has_ignored: bool,
) -> BTreeMap<usize, Vec<TransientStorageChange>> {
    if !has_ignored {
        return transient_storage.clone();
    }

    let mut filtered = BTreeMap::new();
    for (&node_idx, changes) in transient_storage {
        let Some(node) = arena.nodes().get(node_idx) else {
            continue;
        };
        let visible_steps = visible_step_indices(node);
        let retained = changes
            .iter()
            .copied()
            .filter(|change| visible_steps.binary_search(&change.step_idx).is_ok())
            .collect::<Vec<_>>();
        if !retained.is_empty() {
            filtered.insert(node_idx, retained);
        }
    }
    filtered
}

/// Injects compacted `transient storage changes:` sections into a rendered trace string.
///
/// Returns `None` if no node has net transient storage changes to display.
pub(crate) fn inject_transient_storage_changes(
    rendered: &str,
    arena: &CallTraceArena,
    transient_storage: &BTreeMap<usize, Vec<TransientStorageChange>>,
    with_bytecodes: bool,
    has_ignored: bool,
) -> Option<String> {
    if transient_storage.is_empty() {
        return None;
    }

    let mut node_changes = HashMap::<_, _>::default();
    for (&node_idx, changes) in transient_storage {
        let Some(node) = arena.nodes().get(node_idx) else {
            continue;
        };
        let visible_steps = has_ignored.then(|| visible_step_indices(node));
        let compacted = compact_node_transient_changes(node, changes, visible_steps.as_deref());
        if !compacted.is_empty() {
            node_changes.insert(node_idx, compacted);
        }
    }

    if node_changes.is_empty() {
        return None;
    }

    let footers = collect_node_footers(arena, with_bytecodes);
    let mut footer_idx = 0;
    let mut out = String::with_capacity(rendered.len() + node_changes.len() * 64);

    for line in rendered.lines() {
        if let Some(expected) = footers.get(footer_idx)
            && let Some((indent, rest)) = match_edge_prefix(line, expected.indent_level)
            && strip_ansi(rest) == expected.footer_first_line
        {
            footer_idx += 1;
            if let Some(changes) = node_changes.get(&expected.node_idx) {
                let _ = writeln!(out, "{indent}  ├─  transient storage changes:");
                for &(key, before, after) in changes {
                    let _ = writeln!(
                        out,
                        "{indent}  │   @ {}: {} → {}",
                        num_or_hex(key),
                        num_or_hex(before),
                        num_or_hex(after),
                    );
                }
            }
        }
        out.push_str(line);
        out.push('\n');
    }

    if !rendered.ends_with('\n') && out.ends_with('\n') {
        out.pop();
    }

    Some(out)
}

/// Collects the step indices still present in `node.ordering` (strictly ascending).
fn visible_step_indices(node: &CallTraceNode) -> Vec<usize> {
    node.ordering
        .iter()
        .filter_map(|item| match item {
            TraceMemberOrder::Step(step_idx) => Some(*step_idx),
            _ => None,
        })
        .collect()
}

/// Compacts per-node `TSTORE` changes into net `(slot, before, after)` transitions ordered by
/// last write, matching `TraceWriter::write_storage_changes`.
fn compact_node_transient_changes(
    node: &CallTraceNode,
    changes: &[TransientStorageChange],
    visible_steps: Option<&[usize]>,
) -> Vec<(U256, U256, U256)> {
    let mut changes_map = HashMap::<_, _>::default();
    let mut change_id = 0usize;

    for change in changes {
        if change.reason != TransientStorageChangeReason::TSTORE {
            continue;
        }
        if visible_steps.is_some_and(|steps| steps.binary_search(&change.step_idx).is_err()) {
            continue;
        }
        if let Some(step) = node.trace.steps.get(change.step_idx)
            && (step.status.is_some_and(InstrStop::is_halt)
                || matches!(step.decoded(), Some(DecodedTraceStep::Line(_))))
        {
            continue;
        }
        if !change.had_value.is_some_and(|had| had != change.value) {
            continue;
        }

        let (_first, last, last_change_id) =
            changes_map.entry(change.key).or_insert((change, change, change_id));
        *last = change;
        *last_change_id = change_id;
        change_id += 1;
    }

    let mut compacted = changes_map.into_iter().collect::<Vec<_>>();
    compacted.sort_by_key(|(_, (_, _, id))| *id);
    compacted
        .into_iter()
        .filter_map(|(key, (first, last, _))| {
            let before = first.had_value.unwrap_or_default();
            let after = last.value;
            (before != after).then_some((key, before, after))
        })
        .collect()
}

struct NodeFooter {
    node_idx: usize,
    indent_level: usize,
    footer_first_line: String,
}

/// Walks `arena` in the exact order and indentation level that `TraceWriter` uses, including
/// nested [`DecodedTraceStep::InternalCall`] blocks.
fn collect_node_footers(arena: &CallTraceArena, with_bytecodes: bool) -> Vec<NodeFooter> {
    let nodes = arena.nodes();
    if nodes.is_empty() {
        return Vec::new();
    }
    let mut out = Vec::with_capacity(nodes.len());
    walk_node(nodes, 0, 0, with_bytecodes, &mut out);
    out
}

fn walk_node(
    nodes: &[CallTraceNode],
    idx: usize,
    indent_level: usize,
    with_bytecodes: bool,
    out: &mut Vec<NodeFooter>,
) {
    let Some(node) = nodes.get(idx) else {
        return;
    };
    let child_indent = indent_level + 1;
    walk_items_until(nodes, idx, 0, child_indent, with_bytecodes, out, None);
    out.push(NodeFooter {
        node_idx: idx,
        indent_level: child_indent,
        footer_first_line: expected_footer_first_line(&node.trace, with_bytecodes),
    });
}

fn walk_items_until(
    nodes: &[CallTraceNode],
    node_idx: usize,
    mut item_idx: usize,
    indent_level: usize,
    with_bytecodes: bool,
    out: &mut Vec<NodeFooter>,
    stop_step: Option<usize>,
) -> usize {
    let node = &nodes[node_idx];
    while item_idx < node.ordering.len() {
        let item = node.ordering[item_idx];
        if let Some(target_step) = stop_step
            && matches!(item, TraceMemberOrder::Step(step_idx) if step_idx == target_step)
        {
            break;
        }
        match item {
            TraceMemberOrder::Log(_) => item_idx += 1,
            TraceMemberOrder::Call(child_pos) => {
                if let Some(&child_idx) = node.children.get(child_pos) {
                    walk_node(nodes, child_idx, indent_level, with_bytecodes, out);
                }
                item_idx += 1;
            }
            TraceMemberOrder::Step(step_idx) => {
                if let Some(DecodedTraceStep::InternalCall(_, end_step_idx)) =
                    node.trace.steps.get(step_idx).and_then(CallTraceStep::decoded)
                {
                    let end_item_idx = walk_items_until(
                        nodes,
                        node_idx,
                        item_idx + 1,
                        indent_level + 1,
                        with_bytecodes,
                        out,
                        Some(*end_step_idx),
                    );
                    item_idx = end_item_idx + 1;
                } else {
                    item_idx += 1;
                }
            }
        }
    }
    item_idx
}

/// Builds the uncolored first line that `TraceWriter::write_trace_footer` emits for `trace`.
fn expected_footer_first_line(trace: &CallTrace, with_bytecodes: bool) -> String {
    let status = trace.status.unwrap_or(InstrStop::Stop);
    let mut s = format!("← [{status:?}]");
    if let Some(decoded) = trace.decoded.as_deref().and_then(|d| d.return_data.as_deref()) {
        s.push(' ');
        s.push_str(decoded.lines().next().unwrap_or_default());
    } else if !with_bytecodes
        && trace.kind.is_any_create()
        && trace.status.is_none_or(InstrStop::is_success)
    {
        let _ = write!(s, " {} bytes of code", trace.output.len());
    } else if !trace.output.is_empty() {
        let _ = write!(s, " {}", trace.output);
    }
    s
}

/// Matches a `TraceWriter::write_edge` prefix at `indent_level` and returns `(indent, rest)`.
fn match_edge_prefix(line: &str, indent_level: usize) -> Option<(&str, &str)> {
    let mut rest = line.strip_prefix("  ")?;
    for _ in 1..indent_level {
        rest = rest.strip_prefix("  │ ")?;
    }
    let indent_len = line.len() - rest.len();
    let rest = rest.strip_prefix("  └─ ")?;
    Some((&line[..indent_len], rest))
}

/// Strips ANSI CSI SGR escape sequences (`\x1b[...m`) from `s`.
fn strip_ansi(s: &str) -> Cow<'_, str> {
    if !s.contains('\x1b') {
        return Cow::Borrowed(s);
    }
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(esc) = rest.find('\x1b') {
        out.push_str(&rest[..esc]);
        rest = &rest[esc + 1..];
        if let Some(after_csi) = rest.strip_prefix('[')
            && let Some(end) = after_csi.find('m')
        {
            rest = &after_csi[end + 1..];
        } else {
            out.push('\x1b');
        }
    }
    out.push_str(rest);
    Cow::Owned(out)
}

/// Formats `x` as decimal when short and as a 32-byte hex word otherwise, matching
/// `evm2_inspectors::TraceWriter`.
fn num_or_hex(x: U256) -> String {
    if x < U256::from(1_000_000u64) { x.to_string() } else { B256::from(x).to_string() }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        CallTraceDecoder, DecodedInternalCall, SparsedTraceArena, StorageChange,
        StorageChangeReason, decode_trace_arena, render_trace_arena_inner, trace_arena_at_depth,
    };
    use alloy_primitives::Address;
    use evm2::interpreter::opcode::OpCode;
    use snapbox::{assert_data_eq, str};

    fn make_step(op: OpCode, pc: u32, gas_cost: u64) -> CallTraceStep {
        CallTraceStep {
            pc,
            op,
            stack: None,
            push_stack: None,
            memory: None,
            gas_remaining: 0,
            gas_refund_counter: 0,
            gas_cost,
            state_gas_reservoir: None,
            storage_change: None,
            status: None,
            extra: None,
        }
    }

    #[test]
    fn renders_persistent_and_transient_storage_changes() {
        let mut arena = CallTraceArena::default();
        let target = Address::repeat_byte(0x42);
        let root = &mut arena.nodes_mut()[0];
        root.ordering = vec![TraceMemberOrder::Step(0), TraceMemberOrder::Step(1)];
        let mut sstore = make_step(OpCode::SSTORE, 0, 20_000);
        sstore.storage_change = Some(Box::new(StorageChange {
            key: U256::ZERO,
            value: U256::ONE,
            had_value: Some(U256::ZERO),
            reason: StorageChangeReason::SSTORE,
        }));
        let tstore = make_step(OpCode::TSTORE, 1, 100);
        root.trace = CallTrace {
            address: target,
            success: true,
            steps: vec![sstore, tstore],
            ..Default::default()
        };

        let large_slot = U256::from(0x0100_0000u64);
        let traces = SparsedTraceArena {
            arena,
            ignored: Default::default(),
            diagnostics: Default::default(),
            transient_storage: BTreeMap::from([(
                0,
                vec![
                    TransientStorageChange {
                        step_idx: 1,
                        key: U256::from(1),
                        value: U256::from(0xbeef),
                        had_value: Some(U256::ZERO),
                        reason: TransientStorageChangeReason::TSTORE,
                    },
                    TransientStorageChange {
                        step_idx: 1,
                        key: large_slot,
                        value: U256::from(7),
                        had_value: Some(U256::from(2)),
                        reason: TransientStorageChangeReason::TSTORE,
                    },
                ],
            )]),
            debug_steps: None,
        };

        assert_data_eq!(
            render_trace_arena_inner(&traces, false, true),
            str![[r#"
  [0] 0x4242424242424242424242424242424242424242::fallback()
    ├─  storage changes:
    │   @ 0: 0 → 1
    ├─  transient storage changes:
    │   @ 1: 0 → 48879
    │   @ 0x0000000000000000000000000000000000000000000000000000000001000000: 2 → 7
    └─ ← [Stop]

"#]]
        );
    }

    #[test]
    fn compacts_transient_changes_and_omits_net_zero() {
        let mut arena = CallTraceArena::default();
        let target = Address::repeat_byte(0x11);
        let root = &mut arena.nodes_mut()[0];
        root.trace = CallTrace {
            address: target,
            success: true,
            steps: (0..6).map(|pc| make_step(OpCode::TSTORE, pc, 100)).collect(),
            ..Default::default()
        };
        root.ordering = (0..6).map(TraceMemberOrder::Step).collect();

        let traces = SparsedTraceArena {
            arena,
            ignored: Default::default(),
            diagnostics: Default::default(),
            transient_storage: BTreeMap::from([(
                0,
                vec![
                    // Reentrancy guard: 0 -> 1 -> 0 (net zero, omitted).
                    TransientStorageChange {
                        step_idx: 0,
                        key: U256::ZERO,
                        value: U256::ONE,
                        had_value: Some(U256::ZERO),
                        reason: TransientStorageChangeReason::TSTORE,
                    },
                    // Slot 2 written first, then updated after slot 1.
                    TransientStorageChange {
                        step_idx: 1,
                        key: U256::from(2),
                        value: U256::from(10),
                        had_value: Some(U256::ZERO),
                        reason: TransientStorageChangeReason::TSTORE,
                    },
                    TransientStorageChange {
                        step_idx: 2,
                        key: U256::from(1),
                        value: U256::from(20),
                        had_value: Some(U256::ZERO),
                        reason: TransientStorageChangeReason::TSTORE,
                    },
                    TransientStorageChange {
                        step_idx: 3,
                        key: U256::from(2),
                        value: U256::from(30),
                        had_value: Some(U256::from(10)),
                        reason: TransientStorageChangeReason::TSTORE,
                    },
                    // No-op write: 0 -> 0 (omitted).
                    TransientStorageChange {
                        step_idx: 4,
                        key: U256::from(3),
                        value: U256::ZERO,
                        had_value: Some(U256::ZERO),
                        reason: TransientStorageChangeReason::TSTORE,
                    },
                    TransientStorageChange {
                        step_idx: 5,
                        key: U256::ZERO,
                        value: U256::ZERO,
                        had_value: Some(U256::ONE),
                        reason: TransientStorageChangeReason::TSTORE,
                    },
                ],
            )]),
            debug_steps: None,
        };

        assert_data_eq!(
            render_trace_arena_inner(&traces, false, true),
            str![[r#"
  [0] 0x1111111111111111111111111111111111111111::fallback()
    ├─  transient storage changes:
    │   @ 1: 0 → 20
    │   @ 2: 0 → 30
    └─ ← [Stop]

"#]]
        );
    }

    #[test]
    fn renders_transient_changes_with_decoded_internal_calls() {
        let mut arena = CallTraceArena::default();
        let outer = Address::repeat_byte(0xAA);
        let inner = Address::repeat_byte(0xBB);

        let mut start_step = make_step(OpCode::JUMP, 0, 8);
        *start_step.decoded_mut() = DecodedTraceStep::InternalCall(
            DecodedInternalCall { func_name: "helper".into(), args: None, return_data: None },
            1,
        );
        let end_step = make_step(OpCode::JUMPDEST, 1, 1);
        let outer_tstore = make_step(OpCode::TSTORE, 2, 100);

        let root = &mut arena.nodes_mut()[0];
        root.children = vec![1];
        root.ordering = vec![
            TraceMemberOrder::Step(0),
            TraceMemberOrder::Call(0),
            TraceMemberOrder::Step(1),
            TraceMemberOrder::Step(2),
        ];
        root.trace = CallTrace {
            address: outer,
            success: true,
            steps: vec![start_step, end_step, outer_tstore],
            ..Default::default()
        };

        arena.nodes_mut().push(CallTraceNode {
            parent: Some(0),
            idx: 1,
            ordering: vec![TraceMemberOrder::Step(0)],
            trace: CallTrace {
                depth: 1,
                address: inner,
                success: true,
                steps: vec![make_step(OpCode::TSTORE, 0, 100)],
                ..Default::default()
            },
            ..Default::default()
        });

        let traces = SparsedTraceArena {
            arena,
            ignored: Default::default(),
            diagnostics: Default::default(),
            transient_storage: BTreeMap::from([
                (
                    0,
                    vec![TransientStorageChange {
                        step_idx: 2,
                        key: U256::from(1),
                        value: U256::from(99),
                        had_value: Some(U256::ZERO),
                        reason: TransientStorageChangeReason::TSTORE,
                    }],
                ),
                (
                    1,
                    vec![TransientStorageChange {
                        step_idx: 0,
                        key: U256::ZERO,
                        value: U256::from(42),
                        had_value: Some(U256::ZERO),
                        reason: TransientStorageChangeReason::TSTORE,
                    }],
                ),
            ]),
            debug_steps: None,
        };

        assert_data_eq!(
            render_trace_arena_inner(&traces, false, true),
            str![[r#"
  [0] 0xaAaAaAaaAaAaAaaAaAAAAAAAAaaaAaAaAaaAaaAa::fallback()
    ├─ [0] helper
    │   ├─ [0] 0xbBbBBBBbbBBBbbbBbbBbbbbBBbBbbbbBbBbbBBbB::fallback()
    │   │   ├─  transient storage changes:
    │   │   │   @ 0: 0 → 42
    │   │   └─ ← [Stop]
    │   └─ ← 
    ├─  transient storage changes:
    │   @ 1: 0 → 99
    └─ ← [Stop]

"#]]
        );
    }

    #[tokio::test]
    async fn respects_ignored_steps_and_deduplicates_opcode_lines() {
        let mut arena = CallTraceArena::default();
        let target = Address::repeat_byte(0x22);
        let root = &mut arena.nodes_mut()[0];
        root.ordering =
            vec![TraceMemberOrder::Step(0), TraceMemberOrder::Step(1), TraceMemberOrder::Step(2)];
        root.trace = CallTrace {
            address: target,
            success: true,
            steps: vec![
                make_step(OpCode::TSTORE, 0, 100),
                make_step(OpCode::TSTORE, 1, 100),
                make_step(OpCode::TLOAD, 2, 100),
            ],
            ..Default::default()
        };

        let transient_storage = BTreeMap::from([(
            0,
            vec![
                TransientStorageChange {
                    step_idx: 0,
                    key: U256::ZERO,
                    value: U256::from(10),
                    had_value: Some(U256::ZERO),
                    reason: TransientStorageChangeReason::TSTORE,
                },
                TransientStorageChange {
                    step_idx: 1,
                    key: U256::ONE,
                    value: U256::from(20),
                    had_value: Some(U256::ZERO),
                    reason: TransientStorageChangeReason::TSTORE,
                },
                TransientStorageChange {
                    step_idx: 2,
                    key: U256::ONE,
                    value: U256::from(20),
                    had_value: None,
                    reason: TransientStorageChangeReason::TLOAD,
                },
            ],
        )]);

        // Step 0 is ignored via pauseTracing/resumeTracing; only step 1 should show up.
        let mut paused = SparsedTraceArena {
            arena: arena.clone(),
            ignored: HashMap::from_iter([((0, 0), (0, 1))]),
            diagnostics: Default::default(),
            transient_storage: transient_storage.clone(),
            debug_steps: None,
        };
        let mut decoder = CallTraceDecoder::new().clone();
        decoder.opcodes = vec![OpCode::TLOAD];
        decode_trace_arena(&mut paused, &decoder).await;

        assert_data_eq!(
            render_trace_arena_inner(&paused, false, true),
            str![[r#"
  [0] 0x2222222222222222222222222222222222222222::fallback()
    ├─ [100] TLOAD 0x1 → (0x14)
    ├─  transient storage changes:
    │   @ 1: 0 → 20
    └─ ← [Stop]

"#]]
        );

        // When --opcodes TSTORE,TLOAD is active, inline TSTORE lines deduplicate the summary.
        let mut with_tstore = SparsedTraceArena {
            arena,
            ignored: Default::default(),
            diagnostics: Default::default(),
            transient_storage,
            debug_steps: None,
        };
        decoder.opcodes = vec![OpCode::TSTORE, OpCode::TLOAD];
        decode_trace_arena(&mut with_tstore, &decoder).await;

        assert_data_eq!(
            render_trace_arena_inner(&with_tstore, false, true),
            str![[r#"
  [0] 0x2222222222222222222222222222222222222222::fallback()
    ├─ [100] TSTORE 0x0: 0x0 → 0xa
    ├─ [100] TSTORE 0x1: 0x0 → 0x14
    ├─ [100] TLOAD 0x1 → (0x14)
    └─ ← [Stop]

"#]]
        );
    }

    #[test]
    fn strip_ansi_handles_colored_footers() {
        assert_eq!(strip_ansi("\x1b[32m← [Stop]\x1b[0m"), "← [Stop]");
        assert_eq!(strip_ansi("\x1b[31m← [Revert]\x1b[0m custom error"), "← [Revert] custom error");
        assert!(matches!(strip_ansi("← [Stop]"), Cow::Borrowed("← [Stop]")));
    }

    #[test]
    fn serialization_roundtrip_and_depth_projection_preserve_transient_storage() {
        let mut arena = CallTraceArena::default();
        arena.nodes_mut().extend([
            CallTraceNode {
                parent: Some(0),
                children: vec![2],
                idx: 1,
                trace: CallTrace {
                    depth: 1,
                    steps: vec![make_step(OpCode::TSTORE, 0, 100)],
                    ..Default::default()
                },
                ordering: vec![TraceMemberOrder::Step(0), TraceMemberOrder::Call(0)],
                ..Default::default()
            },
            CallTraceNode {
                parent: Some(1),
                idx: 2,
                trace: CallTrace {
                    depth: 2,
                    steps: vec![make_step(OpCode::TSTORE, 0, 100)],
                    ..Default::default()
                },
                ordering: vec![TraceMemberOrder::Step(0)],
                ..Default::default()
            },
            CallTraceNode {
                parent: Some(0),
                idx: 3,
                trace: CallTrace {
                    depth: 1,
                    steps: vec![make_step(OpCode::TSTORE, 0, 100)],
                    ..Default::default()
                },
                ordering: vec![TraceMemberOrder::Step(0)],
                ..Default::default()
            },
        ]);
        let root = &mut arena.nodes_mut()[0];
        root.children = vec![1, 3];
        root.ordering = vec![TraceMemberOrder::Call(0), TraceMemberOrder::Call(1)];

        let change = |v: u64| TransientStorageChange {
            step_idx: 0,
            key: U256::ZERO,
            value: U256::from(v),
            had_value: Some(U256::ZERO),
            reason: TransientStorageChangeReason::TSTORE,
        };
        let traces = SparsedTraceArena {
            arena,
            ignored: Default::default(),
            diagnostics: Default::default(),
            transient_storage: BTreeMap::from([
                (1, vec![change(10)]),
                (2, vec![change(20)]),
                (3, vec![change(30)]),
            ]),
            debug_steps: None,
        };

        // RON roundtrip preserves transient storage.
        let ron_str = ron::to_string(&traces).unwrap();
        let roundtripped: SparsedTraceArena = ron::from_str(&ron_str).unwrap();
        assert_eq!(roundtripped.transient_storage, traces.transient_storage);

        // Depth projection drops node 2 and remaps node 3 -> 2.
        let projected = trace_arena_at_depth(&traces, 1);
        assert_eq!(
            projected.transient_storage,
            BTreeMap::from([(1, vec![change(10)]), (2, vec![change(30)])])
        );

        // Empty transient_storage is omitted from JSON serialization.
        let empty = SparsedTraceArena {
            arena: CallTraceArena::default(),
            ignored: Default::default(),
            diagnostics: Default::default(),
            transient_storage: Default::default(),
            debug_steps: None,
        };
        let json = serde_json::to_string(&empty).unwrap();
        assert!(!json.contains("transient_storage"), "unexpected field in JSON: {json}");
    }
}
