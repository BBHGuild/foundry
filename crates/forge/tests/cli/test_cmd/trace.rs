//! Tests for tracing functionality

use foundry_test_utils::str;

#[forgetest_init]
fn conflicting_signatures(prj: _, cmd: _) {
    prj.add_test(
        "ConflictingSignatures.t.sol",
        r#"
pragma solidity ^0.8.18;

import "forge-std/Test.sol";

contract ReturnsNothing {
    function func() public pure {}
}

contract ReturnsString {
    function func() public pure returns (string memory) {
        return "string";
    }
}

contract ReturnsUint {
    function func() public pure returns (uint256) {
        return 1;
    }
}

contract ConflictingSignaturesTest is Test {
    ReturnsNothing retsNothing;
    ReturnsString retsString;
    ReturnsUint retsUint;

    function setUp() public {
        retsNothing = new ReturnsNothing();
        retsString = new ReturnsString();
        retsUint = new ReturnsUint();
    }

    /// Tests that traces are decoded properly when multiple
    /// functions have the same 4byte signature, but different
    /// return values.
    function testTraceWithConflictingSignatures() public {
        retsNothing.func();
        retsString.func();
        retsUint.func();
    }
}
"#,
    );

    cmd.args(["test", "-vvvvv"]).assert_success().stdout_eq(str![[r#"
...
Ran 1 test for test/ConflictingSignatures.t.sol:ConflictingSignaturesTest
[PASS] testTraceWithConflictingSignatures() ([GAS])
Traces:
  [..] ConflictingSignaturesTest::setUp()
    ├─ [..] → new ReturnsNothing@0x5615dEB798BB3E4dFa0139dFa1b3D433Cc23b72f
    │   └─ ← [Return] 106 bytes of code
    ├─ [..] → new ReturnsString@0x2e234DAe75C793f67A35089C9d99245E1C58470b
    │   └─ ← [Return] 334 bytes of code
    ├─ [..] → new ReturnsUint@0xF62849F9A0B5Bf2913b396098F7c7019b51A820a
    │   └─ ← [Return] 175 bytes of code
    └─ ← [Stop]

  [..] ConflictingSignaturesTest::testTraceWithConflictingSignatures()
    ├─ [..] ReturnsNothing::func() [staticcall]
    │   └─ ← [Stop]
    ├─ [..] ReturnsString::func() [staticcall]
    │   └─ ← [Return] "string"
    ├─ [..] ReturnsUint::func() [staticcall]
    │   └─ ← [Return] 1
    └─ ← [Stop]

Suite result: ok. 1 passed; 0 failed; 0 skipped; [ELAPSED]

Ran 1 test suite [ELAPSED]: 1 tests passed, 0 failed, 0 skipped (1 total tests)

"#]]);
}

#[forgetest_init]
fn trace_struct_outputs_use_called_contract_abi(prj: _, cmd: _) {
    prj.add_test(
        "TraceStructOutputs.t.sol",
        r#"
pragma solidity ^0.8.18;

import "forge-std/Test.sol";

contract AAAMuchWow {
    struct MuchWowStruct {
        uint256 c;
        uint256 d;
    }

    mapping(uint256 => MuchWowStruct) internal items;

    constructor() {
        items[2] = MuchWowStruct({ c: 3, d: 4 });
    }

    function item(uint256 id) public view returns (MuchWowStruct memory) {
        return items[id];
    }
}

contract ZWow {
    struct WowStruct {
        uint256 a;
        uint256 b;
    }

    mapping(uint256 => WowStruct) internal items;

    constructor() {
        items[1] = WowStruct({ a: 1, b: 2 });
    }

    function item(uint256 id) public view returns (WowStruct memory) {
        return items[id];
    }
}

contract TraceStructOutputsTest is Test {
    AAAMuchWow muchWow;
    ZWow wow;

    function setUp() public {
        muchWow = new AAAMuchWow();
        wow = new ZWow();
    }

    function testStructTracesUseContractAbi() public view {
        ZWow.WowStruct memory wowItem = wow.item(1);
        AAAMuchWow.MuchWowStruct memory muchWowItem = muchWow.item(2);

        assertEq(wowItem.a, 1);
        assertEq(wowItem.b, 2);
        assertEq(muchWowItem.c, 3);
        assertEq(muchWowItem.d, 4);
    }
}
"#,
    );

    cmd.args(["test", "--mt", "testStructTracesUseContractAbi", "-vvvv"])
        .assert_success()
        .stdout_eq(str![[r#"
...
Ran 1 test for test/TraceStructOutputs.t.sol:TraceStructOutputsTest
[PASS] testStructTracesUseContractAbi() ([GAS])
Traces:
  [..] TraceStructOutputsTest::testStructTracesUseContractAbi()
    ├─ [..] ZWow::item(1) [staticcall]
    │   └─ ← [Return] WowStruct({ a: 1, b: 2 })
    ├─ [..] AAAMuchWow::item(2) [staticcall]
    │   └─ ← [Return] MuchWowStruct({ c: 3, d: 4 })
    └─ ← [Stop]

Suite result: ok. 1 passed; 0 failed; 0 skipped; [ELAPSED]

Ran 1 test suite [ELAPSED]: 1 tests passed, 0 failed, 0 skipped (1 total tests)

"#]]);
}

#[forgetest_init]
fn trace_event_struct_with_function_pointer(prj: _, cmd: _) {
    prj.add_test(
        "StructWithFunctionEvent.t.sol",
        r#"
// SPDX-License-Identifier: MIT
pragma solidity ^0.8.28;

contract StructWithFunctionEvent {
    struct Action {
        uint256 id;
        function(uint256) external callback;
    }

    uint256 public i;

    event ActionLogged(Action action);

    function sampleCallback(uint256 value) external {
        i += value;
    }

    function testEmitFunctionInStruct() external {
        Action memory myAction = Action({id: 1337, callback: this.sampleCallback});
        emit ActionLogged(myAction);
    }
}
"#,
    );

    cmd.args(["test", "--mt", "testEmitFunctionInStruct", "-vvvv"]).assert_success().stdout_eq(
        str![[r#"
[COMPILING_FILES] with [SOLC_VERSION]
[SOLC_VERSION] [ELAPSED]
Compiler run successful!

Ran 1 test for test/StructWithFunctionEvent.t.sol:StructWithFunctionEvent
[PASS] testEmitFunctionInStruct() ([GAS])
Traces:
  [..] StructWithFunctionEvent::testEmitFunctionInStruct()
    ├─ emit ActionLogged(action: Action({ id: 1337, callback: 0x[..]7cfdab90 }))
    └─ ← [Stop]

Suite result: ok. 1 passed; 0 failed; 0 skipped; [ELAPSED]

Ran 1 test suite [ELAPSED]: 1 tests passed, 0 failed, 0 skipped (1 total tests)

"#]],
    );
}

#[forgetest_init]
fn trace_test(prj: _, cmd: _) {
    prj.add_test(
        "Trace.t.sol",
        r#"
pragma solidity ^0.8.18;

import "forge-std/Test.sol";

contract RecursiveCall {
    TraceTest factory;

    event Depth(uint256 depth);
    event ChildDepth(uint256 childDepth);
    event CreatedChild(uint256 childDepth);

    constructor(address _factory) {
        factory = TraceTest(_factory);
    }

    function recurseCall(uint256 neededDepth, uint256 depth) public returns (uint256) {
        if (depth == neededDepth) {
            this.negativeNum();
            return neededDepth;
        }

        uint256 childDepth = this.recurseCall(neededDepth, depth + 1);
        emit ChildDepth(childDepth);

        this.someCall();
        emit Depth(depth);

        return depth;
    }

    function recurseCreate(uint256 neededDepth, uint256 depth) public returns (uint256) {
        if (depth == neededDepth) {
            return neededDepth;
        }

        RecursiveCall child = factory.create();
        emit CreatedChild(depth + 1);

        uint256 childDepth = child.recurseCreate(neededDepth, depth + 1);
        emit ChildDepth(childDepth);
        emit Depth(depth);

        return depth;
    }

    function someCall() public pure {}

    function negativeNum() public pure returns (int256) {
        return -1000000000;
    }
}

contract TraceTest is Test {
    uint256 nodeId = 0;
    RecursiveCall first;

    function setUp() public {
        first = this.create();
    }

    function create() public returns (RecursiveCall) {
        RecursiveCall node = new RecursiveCall(address(this));
        vm.label(address(node), string(abi.encodePacked("Node ", uintToString(nodeId++))));

        return node;
    }

    function testRecurseCall() public {
        first.recurseCall(8, 0);
    }

    function testRecurseCreate() public {
        first.recurseCreate(8, 0);
    }
}

function uintToString(uint256 value) pure returns (string memory) {
    // Taken from OpenZeppelin
    if (value == 0) {
        return "0";
    }
    uint256 temp = value;
    uint256 digits;
    while (temp != 0) {
        digits++;
        temp /= 10;
    }
    bytes memory buffer = new bytes(digits);
    while (value != 0) {
        digits -= 1;
        buffer[digits] = bytes1(uint8(48 + uint256(value % 10)));
        value /= 10;
    }
    return string(buffer);
}
"#,
    );

    cmd.args(["test", "-vvvvv"]).assert_success().stdout_eq(str![[r#"
...
Ran 2 tests for test/Trace.t.sol:TraceTest
[PASS] testRecurseCall() ([GAS])
Traces:
  [..] TraceTest::setUp()
    ├─ [..] TraceTest::create()
    │   ├─ [405132] → new Node 0@0x2e234DAe75C793f67A35089C9d99245E1C58470b
    │   │   └─ ← [Return] 1911 bytes of code
    │   ├─ [0] VM::label(Node 0: [0x2e234DAe75C793f67A35089C9d99245E1C58470b], "Node 0")
    │   │   └─ ← [Return]
    │   └─ ← [Return] Node 0: [0x2e234DAe75C793f67A35089C9d99245E1C58470b]
    └─ ← [Stop]

  [..] TraceTest::testRecurseCall()
    ├─ [..] Node 0::recurseCall(8, 0)
    │   ├─ [..] Node 0::recurseCall(8, 1)
    │   │   ├─ [..] Node 0::recurseCall(8, 2)
    │   │   │   ├─ [..] Node 0::recurseCall(8, 3)
    │   │   │   │   ├─ [..] Node 0::recurseCall(8, 4)
    │   │   │   │   │   ├─ [..] Node 0::recurseCall(8, 5)
    │   │   │   │   │   │   ├─ [..] Node 0::recurseCall(8, 6)
    │   │   │   │   │   │   │   ├─ [..] Node 0::recurseCall(8, 7)
    │   │   │   │   │   │   │   │   ├─ [..] Node 0::recurseCall(8, 8)
    │   │   │   │   │   │   │   │   │   ├─ [..] Node 0::negativeNum() [staticcall]
    │   │   │   │   │   │   │   │   │   │   └─ ← [Return] -1000000000 [-1e9]
    │   │   │   │   │   │   │   │   │   └─ ← [Return] 8
    │   │   │   │   │   │   │   │   ├─ emit ChildDepth(childDepth: 8)
    │   │   │   │   │   │   │   │   ├─ [..] Node 0::someCall() [staticcall]
    │   │   │   │   │   │   │   │   │   └─ ← [Stop]
    │   │   │   │   │   │   │   │   ├─ emit Depth(depth: 7)
    │   │   │   │   │   │   │   │   └─ ← [Return] 7
    │   │   │   │   │   │   │   ├─ emit ChildDepth(childDepth: 7)
    │   │   │   │   │   │   │   ├─ [..] Node 0::someCall() [staticcall]
    │   │   │   │   │   │   │   │   └─ ← [Stop]
    │   │   │   │   │   │   │   ├─ emit Depth(depth: 6)
    │   │   │   │   │   │   │   └─ ← [Return] 6
    │   │   │   │   │   │   ├─ emit ChildDepth(childDepth: 6)
    │   │   │   │   │   │   ├─ [..] Node 0::someCall() [staticcall]
    │   │   │   │   │   │   │   └─ ← [Stop]
    │   │   │   │   │   │   ├─ emit Depth(depth: 5)
    │   │   │   │   │   │   └─ ← [Return] 5
    │   │   │   │   │   ├─ emit ChildDepth(childDepth: 5)
    │   │   │   │   │   ├─ [..] Node 0::someCall() [staticcall]
    │   │   │   │   │   │   └─ ← [Stop]
    │   │   │   │   │   ├─ emit Depth(depth: 4)
    │   │   │   │   │   └─ ← [Return] 4
    │   │   │   │   ├─ emit ChildDepth(childDepth: 4)
    │   │   │   │   ├─ [..] Node 0::someCall() [staticcall]
    │   │   │   │   │   └─ ← [Stop]
    │   │   │   │   ├─ emit Depth(depth: 3)
    │   │   │   │   └─ ← [Return] 3
    │   │   │   ├─ emit ChildDepth(childDepth: 3)
    │   │   │   ├─ [..] Node 0::someCall() [staticcall]
    │   │   │   │   └─ ← [Stop]
    │   │   │   ├─ emit Depth(depth: 2)
    │   │   │   └─ ← [Return] 2
    │   │   ├─ emit ChildDepth(childDepth: 2)
    │   │   ├─ [..] Node 0::someCall() [staticcall]
    │   │   │   └─ ← [Stop]
    │   │   ├─ emit Depth(depth: 1)
    │   │   └─ ← [Return] 1
    │   ├─ emit ChildDepth(childDepth: 1)
    │   ├─ [..] Node 0::someCall() [staticcall]
    │   │   └─ ← [Stop]
    │   ├─ emit Depth(depth: 0)
    │   └─ ← [Return] 0
    └─ ← [Stop]

[PASS] testRecurseCreate() ([GAS])
Traces:
  [..] TraceTest::setUp()
    ├─ [..] TraceTest::create()
    │   ├─ [405132] → new Node 0@0x2e234DAe75C793f67A35089C9d99245E1C58470b
    │   │   └─ ← [Return] 1911 bytes of code
    │   ├─ [0] VM::label(Node 0: [0x2e234DAe75C793f67A35089C9d99245E1C58470b], "Node 0")
    │   │   └─ ← [Return]
    │   └─ ← [Return] Node 0: [0x2e234DAe75C793f67A35089C9d99245E1C58470b]
    └─ ← [Stop]

  [..] TraceTest::testRecurseCreate()
    ├─ [..] Node 0::recurseCreate(8, 0)
    │   ├─ [..] TraceTest::create()
    │   │   ├─ [405132] → new Node 1@0x5991A2dF15A8F6A256D3Ec51E99254Cd3fb576A9
    │   │   │   ├─  storage changes:
    │   │   │   │   @ 0: 0 → 0x0000000000000000000000007fa9385be102ac3eac297483dd6233d62b3e1496
    │   │   │   └─ ← [Return] 1911 bytes of code
    │   │   ├─ [0] VM::label(Node 1: [0x5991A2dF15A8F6A256D3Ec51E99254Cd3fb576A9], "Node 1")
    │   │   │   └─ ← [Return]
    │   │   ├─  storage changes:
    │   │   │   @ 32: 1 → 2
    │   │   └─ ← [Return] Node 1: [0x5991A2dF15A8F6A256D3Ec51E99254Cd3fb576A9]
    │   ├─ emit CreatedChild(childDepth: 1)
    │   ├─ [..] Node 1::recurseCreate(8, 1)
    │   │   ├─ [..] TraceTest::create()
    │   │   │   ├─ [405132] → new Node 2@0xc7183455a4C133Ae270771860664b6B7ec320bB1
    │   │   │   │   ├─  storage changes:
    │   │   │   │   │   @ 0: 0 → 0x0000000000000000000000007fa9385be102ac3eac297483dd6233d62b3e1496
    │   │   │   │   └─ ← [Return] 1911 bytes of code
    │   │   │   ├─ [0] VM::label(Node 2: [0xc7183455a4C133Ae270771860664b6B7ec320bB1], "Node 2")
    │   │   │   │   └─ ← [Return]
    │   │   │   ├─  storage changes:
    │   │   │   │   @ 32: 2 → 3
    │   │   │   └─ ← [Return] Node 2: [0xc7183455a4C133Ae270771860664b6B7ec320bB1]
    │   │   ├─ emit CreatedChild(childDepth: 2)
    │   │   ├─ [..] Node 2::recurseCreate(8, 2)
    │   │   │   ├─ [..] TraceTest::create()
    │   │   │   │   ├─ [405132] → new Node 3@0xa0Cb889707d426A7A386870A03bc70d1b0697598
    │   │   │   │   │   ├─  storage changes:
    │   │   │   │   │   │   @ 0: 0 → 0x0000000000000000000000007fa9385be102ac3eac297483dd6233d62b3e1496
    │   │   │   │   │   └─ ← [Return] 1911 bytes of code
    │   │   │   │   ├─ [0] VM::label(Node 3: [0xa0Cb889707d426A7A386870A03bc70d1b0697598], "Node 3")
    │   │   │   │   │   └─ ← [Return]
    │   │   │   │   ├─  storage changes:
    │   │   │   │   │   @ 32: 3 → 4
    │   │   │   │   └─ ← [Return] Node 3: [0xa0Cb889707d426A7A386870A03bc70d1b0697598]
    │   │   │   ├─ emit CreatedChild(childDepth: 3)
    │   │   │   ├─ [..] Node 3::recurseCreate(8, 3)
    │   │   │   │   ├─ [..] TraceTest::create()
    │   │   │   │   │   ├─ [405132] → new Node 4@0x1d1499e622D69689cdf9004d05Ec547d650Ff211
    │   │   │   │   │   │   ├─  storage changes:
    │   │   │   │   │   │   │   @ 0: 0 → 0x0000000000000000000000007fa9385be102ac3eac297483dd6233d62b3e1496
    │   │   │   │   │   │   └─ ← [Return] 1911 bytes of code
    │   │   │   │   │   ├─ [0] VM::label(Node 4: [0x1d1499e622D69689cdf9004d05Ec547d650Ff211], "Node 4")
    │   │   │   │   │   │   └─ ← [Return]
    │   │   │   │   │   ├─  storage changes:
    │   │   │   │   │   │   @ 32: 4 → 5
    │   │   │   │   │   └─ ← [Return] Node 4: [0x1d1499e622D69689cdf9004d05Ec547d650Ff211]
    │   │   │   │   ├─ emit CreatedChild(childDepth: 4)
    │   │   │   │   ├─ [..] Node 4::recurseCreate(8, 4)
    │   │   │   │   │   ├─ [..] TraceTest::create()
    │   │   │   │   │   │   ├─ [405132] → new Node 5@0xA4AD4f68d0b91CFD19687c881e50f3A00242828c
    │   │   │   │   │   │   │   ├─  storage changes:
    │   │   │   │   │   │   │   │   @ 0: 0 → 0x0000000000000000000000007fa9385be102ac3eac297483dd6233d62b3e1496
    │   │   │   │   │   │   │   └─ ← [Return] 1911 bytes of code
    │   │   │   │   │   │   ├─ [0] VM::label(Node 5: [0xA4AD4f68d0b91CFD19687c881e50f3A00242828c], "Node 5")
    │   │   │   │   │   │   │   └─ ← [Return]
    │   │   │   │   │   │   ├─  storage changes:
    │   │   │   │   │   │   │   @ 32: 5 → 6
    │   │   │   │   │   │   └─ ← [Return] Node 5: [0xA4AD4f68d0b91CFD19687c881e50f3A00242828c]
    │   │   │   │   │   ├─ emit CreatedChild(childDepth: 5)
    │   │   │   │   │   ├─ [..] Node 5::recurseCreate(8, 5)
    │   │   │   │   │   │   ├─ [..] TraceTest::create()
    │   │   │   │   │   │   │   ├─ [405132] → new Node 6@0x03A6a84cD762D9707A21605b548aaaB891562aAb
    │   │   │   │   │   │   │   │   ├─  storage changes:
    │   │   │   │   │   │   │   │   │   @ 0: 0 → 0x0000000000000000000000007fa9385be102ac3eac297483dd6233d62b3e1496
    │   │   │   │   │   │   │   │   └─ ← [Return] 1911 bytes of code
    │   │   │   │   │   │   │   ├─ [0] VM::label(Node 6: [0x03A6a84cD762D9707A21605b548aaaB891562aAb], "Node 6")
    │   │   │   │   │   │   │   │   └─ ← [Return]
    │   │   │   │   │   │   │   ├─  storage changes:
    │   │   │   │   │   │   │   │   @ 32: 6 → 7
    │   │   │   │   │   │   │   └─ ← [Return] Node 6: [0x03A6a84cD762D9707A21605b548aaaB891562aAb]
    │   │   │   │   │   │   ├─ emit CreatedChild(childDepth: 6)
    │   │   │   │   │   │   ├─ [..] Node 6::recurseCreate(8, 6)
    │   │   │   │   │   │   │   ├─ [..] TraceTest::create()
    │   │   │   │   │   │   │   │   ├─ [405132] → new Node 7@0xD6BbDE9174b1CdAa358d2Cf4D57D1a9F7178FBfF
    │   │   │   │   │   │   │   │   │   ├─  storage changes:
    │   │   │   │   │   │   │   │   │   │   @ 0: 0 → 0x0000000000000000000000007fa9385be102ac3eac297483dd6233d62b3e1496
    │   │   │   │   │   │   │   │   │   └─ ← [Return] 1911 bytes of code
    │   │   │   │   │   │   │   │   ├─ [0] VM::label(Node 7: [0xD6BbDE9174b1CdAa358d2Cf4D57D1a9F7178FBfF], "Node 7")
    │   │   │   │   │   │   │   │   │   └─ ← [Return]
    │   │   │   │   │   │   │   │   ├─  storage changes:
    │   │   │   │   │   │   │   │   │   @ 32: 7 → 8
    │   │   │   │   │   │   │   │   └─ ← [Return] Node 7: [0xD6BbDE9174b1CdAa358d2Cf4D57D1a9F7178FBfF]
    │   │   │   │   │   │   │   ├─ emit CreatedChild(childDepth: 7)
    │   │   │   │   │   │   │   ├─ [..] Node 7::recurseCreate(8, 7)
    │   │   │   │   │   │   │   │   ├─ [..] TraceTest::create()
    │   │   │   │   │   │   │   │   │   ├─ [405132] → new Node 8@0x15cF58144EF33af1e14b5208015d11F9143E27b9
    │   │   │   │   │   │   │   │   │   │   ├─  storage changes:
    │   │   │   │   │   │   │   │   │   │   │   @ 0: 0 → 0x0000000000000000000000007fa9385be102ac3eac297483dd6233d62b3e1496
    │   │   │   │   │   │   │   │   │   │   └─ ← [Return] 1911 bytes of code
    │   │   │   │   │   │   │   │   │   ├─ [0] VM::label(Node 8: [0x15cF58144EF33af1e14b5208015d11F9143E27b9], "Node 8")
    │   │   │   │   │   │   │   │   │   │   └─ ← [Return]
    │   │   │   │   │   │   │   │   │   ├─  storage changes:
    │   │   │   │   │   │   │   │   │   │   @ 32: 8 → 9
    │   │   │   │   │   │   │   │   │   └─ ← [Return] Node 8: [0x15cF58144EF33af1e14b5208015d11F9143E27b9]
    │   │   │   │   │   │   │   │   ├─ emit CreatedChild(childDepth: 8)
    │   │   │   │   │   │   │   │   ├─ [..] Node 8::recurseCreate(8, 8)
    │   │   │   │   │   │   │   │   │   └─ ← [Return] 8
    │   │   │   │   │   │   │   │   ├─ emit ChildDepth(childDepth: 8)
    │   │   │   │   │   │   │   │   ├─ emit Depth(depth: 7)
    │   │   │   │   │   │   │   │   └─ ← [Return] 7
    │   │   │   │   │   │   │   ├─ emit ChildDepth(childDepth: 7)
    │   │   │   │   │   │   │   ├─ emit Depth(depth: 6)
    │   │   │   │   │   │   │   └─ ← [Return] 6
    │   │   │   │   │   │   ├─ emit ChildDepth(childDepth: 6)
    │   │   │   │   │   │   ├─ emit Depth(depth: 5)
    │   │   │   │   │   │   └─ ← [Return] 5
    │   │   │   │   │   ├─ emit ChildDepth(childDepth: 5)
    │   │   │   │   │   ├─ emit Depth(depth: 4)
    │   │   │   │   │   └─ ← [Return] 4
    │   │   │   │   ├─ emit ChildDepth(childDepth: 4)
    │   │   │   │   ├─ emit Depth(depth: 3)
    │   │   │   │   └─ ← [Return] 3
    │   │   │   ├─ emit ChildDepth(childDepth: 3)
    │   │   │   ├─ emit Depth(depth: 2)
    │   │   │   └─ ← [Return] 2
    │   │   ├─ emit ChildDepth(childDepth: 2)
    │   │   ├─ emit Depth(depth: 1)
    │   │   └─ ← [Return] 1
    │   ├─ emit ChildDepth(childDepth: 1)
    │   ├─ emit Depth(depth: 0)
    │   └─ ← [Return] 0
    └─ ← [Stop]

Suite result: ok. 2 passed; 0 failed; 0 skipped; [ELAPSED]

Ran 1 test suite [ELAPSED]: 2 tests passed, 0 failed, 0 skipped (2 total tests)

"#]]);
}

#[forgetest_init]
fn trace_test_detph(prj: _, cmd: _) {
    prj.add_test(
        "Trace.t.sol",
        r#"
pragma solidity ^0.8.18;

import "forge-std/Test.sol";

contract RecursiveCall {
    TraceTest factory;

    event Depth(uint256 depth);
    event ChildDepth(uint256 childDepth);
    event CreatedChild(uint256 childDepth);

    constructor(address _factory) {
        factory = TraceTest(_factory);
    }

    function recurseCall(uint256 neededDepth, uint256 depth) public returns (uint256) {
        if (depth == neededDepth) {
            this.negativeNum();
            return neededDepth;
        }

        uint256 childDepth = this.recurseCall(neededDepth, depth + 1);
        emit ChildDepth(childDepth);

        this.someCall();
        emit Depth(depth);

        return depth;
    }

    function recurseCreate(uint256 neededDepth, uint256 depth) public returns (uint256) {
        if (depth == neededDepth) {
            return neededDepth;
        }

        RecursiveCall child = factory.create();
        emit CreatedChild(depth + 1);

        uint256 childDepth = child.recurseCreate(neededDepth, depth + 1);
        emit ChildDepth(childDepth);
        emit Depth(depth);

        return depth;
    }

    function someCall() public pure {}

    function negativeNum() public pure returns (int256) {
        return -1000000000;
    }
}

contract TraceTest is Test {
    uint256 nodeId = 0;
    RecursiveCall first;

    function setUp() public {
        first = this.create();
    }

    function create() public returns (RecursiveCall) {
        RecursiveCall node = new RecursiveCall(address(this));
        vm.label(address(node), string(abi.encodePacked("Node ", uintToString(nodeId++))));

        return node;
    }

    function testRecurseCall() public {
        first.recurseCall(8, 0);
    }

    function testRecurseCreate() public {
        first.recurseCreate(8, 0);
    }
}

function uintToString(uint256 value) pure returns (string memory) {
    // Taken from OpenZeppelin
    if (value == 0) {
        return "0";
    }
    uint256 temp = value;
    uint256 digits;
    while (temp != 0) {
        digits++;
        temp /= 10;
    }
    bytes memory buffer = new bytes(digits);
    while (value != 0) {
        digits -= 1;
        buffer[digits] = bytes1(uint8(48 + uint256(value % 10)));
        value /= 10;
    }
    return string(buffer);
}
"#,
    );

    cmd.args(["test", "-vvvvv", "--trace-depth", "3"]).assert_success().stdout_eq(str![[r#"
...
Ran 2 tests for test/Trace.t.sol:TraceTest
[PASS] testRecurseCall() ([GAS])
Traces:
  [..] TraceTest::setUp()
    ├─ [..] TraceTest::create()
    │   ├─ [405132] → new Node 0@0x2e234DAe75C793f67A35089C9d99245E1C58470b
    │   │   └─ ← [Return] 1911 bytes of code
    │   ├─ [0] VM::label(Node 0: [0x2e234DAe75C793f67A35089C9d99245E1C58470b], "Node 0")
    │   │   └─ ← [Return]
    │   └─ ← [Return] Node 0: [0x2e234DAe75C793f67A35089C9d99245E1C58470b]
    └─ ← [Stop]

  [..] TraceTest::testRecurseCall()
    ├─ [..] Node 0::recurseCall(8, 0)
    │   ├─ [..] Node 0::recurseCall(8, 1)
    │   │   ├─ [..] Node 0::recurseCall(8, 2)
    │   │   │   └─ ← [Return] 2
    │   │   ├─ emit ChildDepth(childDepth: 2)
    │   │   ├─ [..] Node 0::someCall() [staticcall]
    │   │   │   └─ ← [Stop]
    │   │   ├─ emit Depth(depth: 1)
    │   │   └─ ← [Return] 1
    │   ├─ emit ChildDepth(childDepth: 1)
    │   ├─ [..] Node 0::someCall() [staticcall]
    │   │   └─ ← [Stop]
    │   ├─ emit Depth(depth: 0)
    │   └─ ← [Return] 0
    └─ ← [Stop]

[PASS] testRecurseCreate() ([GAS])
Traces:
  [..] TraceTest::setUp()
    ├─ [..] TraceTest::create()
    │   ├─ [405132] → new Node 0@0x2e234DAe75C793f67A35089C9d99245E1C58470b
    │   │   └─ ← [Return] 1911 bytes of code
    │   ├─ [0] VM::label(Node 0: [0x2e234DAe75C793f67A35089C9d99245E1C58470b], "Node 0")
    │   │   └─ ← [Return]
    │   └─ ← [Return] Node 0: [0x2e234DAe75C793f67A35089C9d99245E1C58470b]
    └─ ← [Stop]

  [..] TraceTest::testRecurseCreate()
    ├─ [..] Node 0::recurseCreate(8, 0)
    │   ├─ [..] TraceTest::create()
    │   │   ├─ [405132] → new Node 1@0x5991A2dF15A8F6A256D3Ec51E99254Cd3fb576A9
    │   │   │   ├─  storage changes:
    │   │   │   │   @ 0: 0 → 0x0000000000000000000000007fa9385be102ac3eac297483dd6233d62b3e1496
    │   │   │   └─ ← [Return] 1911 bytes of code
    │   │   ├─ [0] VM::label(Node 1: [0x5991A2dF15A8F6A256D3Ec51E99254Cd3fb576A9], "Node 1")
    │   │   │   └─ ← [Return]
    │   │   ├─  storage changes:
    │   │   │   @ 32: 1 → 2
    │   │   └─ ← [Return] Node 1: [0x5991A2dF15A8F6A256D3Ec51E99254Cd3fb576A9]
    │   ├─ emit CreatedChild(childDepth: 1)
    │   ├─ [..] Node 1::recurseCreate(8, 1)
    │   │   ├─ [..] TraceTest::create()
    │   │   │   ├─  storage changes:
    │   │   │   │   @ 32: 2 → 3
    │   │   │   └─ ← [Return] Node 2: [0xc7183455a4C133Ae270771860664b6B7ec320bB1]
    │   │   ├─ emit CreatedChild(childDepth: 2)
    │   │   ├─ [..] Node 2::recurseCreate(8, 2)
    │   │   │   └─ ← [Return] 2
    │   │   ├─ emit ChildDepth(childDepth: 2)
    │   │   ├─ emit Depth(depth: 1)
    │   │   └─ ← [Return] 1
    │   ├─ emit ChildDepth(childDepth: 1)
    │   ├─ emit Depth(depth: 0)
    │   └─ ← [Return] 0
    └─ ← [Stop]

Suite result: ok. 2 passed; 0 failed; 0 skipped; [ELAPSED]

Ran 1 test suite [ELAPSED]: 2 tests passed, 0 failed, 0 skipped (2 total tests)

"#]]);
}

// Tests that deploy helpers generated by dynamic test linking are not reported as removed sources.
#[forgetest_init]
fn no_removed_source_warning_for_deploy_helpers(prj: _, cmd: _) {
    prj.add_source(
        "Greeter.sol",
        r#"
pragma solidity ^0.8.18;

contract Greeter {
    uint256 public number;

    constructor(uint256 _number) {
        number = _number;
    }
}
"#,
    );
    prj.add_test(
        "Greeter.t.sol",
        r#"
pragma solidity ^0.8.18;

import {Test} from "forge-std/Test.sol";
import {Greeter} from "../src/Greeter.sol";

contract GreeterTest is Test {
    function test_deploy() public {
        assertEq(new Greeter(1).number(), 1);
    }
}
"#,
    );

    cmd.args(["test", "--decode-internal", "--mt", "test_deploy"])
        .assert_success()
        .stderr_eq(str![""]);
}

// Nested transactions must share the suspended trace depth and restore isolation context.
#[forgetest_init]
fn execute_transaction_nested_tracing(prj: _, cmd: _) {
    prj.add_test(
        "ExecuteTransactionTracing.t.sol",
        &include_str!("../../../../../testdata/default/cheats/ExecuteTransactionTracing.t.sol")
            .replace("utils/Test.sol", "forge-std/Test.sol"),
    );

    for isolate in [false, true] {
        cmd.forge_fuse();
        cmd.env("FOUNDRY_ISOLATE", isolate.to_string());
        cmd.args(["test", "-vvv"]).assert_success().stdout_eq(str![[r#"
...
Ran 9 tests for test/ExecuteTransactionTracing.t.sol:ExecuteTransactionTracingTest
[PASS] test_broadcast_raw_subcall() ([GAS])
[PASS] test_nested_create() ([GAS])
[PASS] test_nested_halt() ([GAS])
[PASS] test_nested_invalid_nonce() ([GAS])
[PASS] test_nested_leaf() ([GAS])
[PASS] test_nested_revert() ([GAS])
[PASS] test_nested_subcall() ([GAS])
[PASS] test_nested_transaction_creates_child() ([GAS])
[PASS] test_recursive_transaction() ([GAS])
Suite result: ok. 9 passed; 0 failed; 0 skipped; [ELAPSED]

Ran 1 test suite [ELAPSED]: 9 tests passed, 0 failed, 0 skipped (9 total tests)

"#]]);

        cmd.forge_fuse();
        cmd.env("FOUNDRY_ISOLATE", isolate.to_string());
        cmd.args(["test", "--mt", "test_nested_subcall", "-vvvv"]).assert_success().stdout_eq(
            str![[r#"
No files changed, compilation skipped

Ran 1 test for test/ExecuteTransactionTracing.t.sol:ExecuteTransactionTracingTest
[PASS] test_nested_subcall() ([GAS])
Traces:
  [..] ExecuteTransactionTracingTest::test_nested_subcall()
...
    ├─ [..] ExecuteTransactionTracingTest::execute([..])
    │   ├─ [..] VM::executeTransaction([..])
    │   │   ├─ [..] ExecuteTransactionTracingTest::leaf()
    │   │   │   └─ ← [Stop]
    │   │   └─ ← [Return] 0x
    │   ├─ [..] ExecuteTransactionTracingTest::leaf()
    │   │   └─ ← [Stop]
    │   └─ ← [Stop]
    ├─ [..] ExecuteTransactionTracingTest::leaf()
    │   └─ ← [Stop]
    └─ ← [Stop]

Suite result: ok. 1 passed; 0 failed; 0 skipped; [ELAPSED]

Ran 1 test suite [ELAPSED]: 1 tests passed, 0 failed, 0 skipped (1 total tests)

"#]],
        );

        // A raw transaction's root call nests below the cheatcode call.
        cmd.forge_fuse();
        cmd.env("FOUNDRY_ISOLATE", isolate.to_string());
        cmd.args(["test", "--mt", "test_broadcast_raw_subcall", "-vvvv"])
            .assert_success()
            .stdout_eq(str![[r#"
No files changed, compilation skipped

Ran 1 test for test/ExecuteTransactionTracing.t.sol:ExecuteTransactionTracingTest
[PASS] test_broadcast_raw_subcall() ([GAS])
Traces:
  [..] ExecuteTransactionTracingTest::test_broadcast_raw_subcall()
...
    ├─ [..] ExecuteTransactionTracingTest::broadcast([..])
    │   ├─ [..] VM::broadcastRawTransaction([..])
    │   │   ├─ [..] ExecuteTransactionTracingTest::subcall()
    │   │   │   ├─ [..] ExecuteTransactionTracingTest::leaf()
    │   │   │   │   └─ ← [Stop]
    │   │   │   └─ ← [Stop]
    │   │   └─ ← [Return]
    │   ├─ [..] ExecuteTransactionTracingTest::leaf()
    │   │   └─ ← [Stop]
    │   └─ ← [Stop]
    ├─ [..] ExecuteTransactionTracingTest::leaf()
    │   └─ ← [Stop]
    └─ ← [Stop]

Suite result: ok. 1 passed; 0 failed; 0 skipped; [ELAPSED]

Ran 1 test suite [ELAPSED]: 1 tests passed, 0 failed, 0 skipped (1 total tests)

"#]]);
    }
}

// Library ABIs omit external functions with storage parameters. A revert in such a function must
// not show as an unrecognized function selector.
#[forgetest]
fn library_storage_param_revert(prj: _, cmd: _) {
    prj.add_source(
        "Lib.sol",
        r#"
library Lib {
    struct S {
        uint256 x;
    }

    uint256 public constant MAX = 10;

    function check(S storage s, uint256 y) external view returns (uint256) {
        if (s.x + y > MAX) {
            assembly {
                revert(0, 0)
            }
        }
        return s.x + y;
    }
}

contract Box {
    Lib.S s;

    function run(uint256 y) external view returns (uint256) {
        return Lib.check(s, y);
    }
}
"#,
    );
    prj.add_test(
        "Box.t.sol",
        r#"
import "../src/Lib.sol";

contract BoxTest {
    function test_run() public {
        new Box().run(11);
    }
}
"#,
    );

    for dynamic_test_linking in [false, true] {
        prj.update_config(|config| config.dynamic_test_linking = dynamic_test_linking);
        cmd.forge_fuse().args(["test", "-vvvv"]).assert_failure().stdout_eq(str![[r#"
...
    ├─ [..] Box::run(11) [staticcall]
    │   ├─ [..] Lib::52f5004a([..]) [delegatecall]
    │   │   └─ ← [Revert] EvmError: Revert
    │   └─ ← [Revert] EvmError: Revert
    └─ ← [Revert] EvmError: Revert
...
"#]]);
    }
}

#[forgetest_init]
fn transient_storage_traces_and_opcodes(prj: _, cmd: _) {
    prj.add_source(
        "TransientDemo.sol",
        r#"
// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

contract TransientHelper {
    function bump(uint256 slot, uint256 value) external {
        assembly {
            tstore(slot, value)
        }
    }
}

contract TransientCtor {
    constructor(uint256 value) {
        assembly {
            tstore(5, value)
        }
    }
}

contract TransientDemo {
    uint256 public persisted;

    function storeBothAndCompact(address helper) external returns (uint256 loaded) {
        persisted = 1;
        assembly {
            // Net-zero reentrancy guard: 0 -> 1 -> 0 (omitted from summary).
            tstore(0, 1)
            // Compacted writes: slot 2 written first, then updated after slot 1.
            tstore(2, 10)
            tstore(1, 20)
            tstore(2, 30)
            // Read back slot 2.
            loaded := tload(2)
            tstore(0, 0)
        }
        // Delegatecall modifies caller's transient slot 2 (30 -> 40).
        (bool ok,) = helper.delegatecall(abi.encodeCall(TransientHelper.bump, (2, 40)));
        require(ok);
        // Constructor transient write.
        new TransientCtor(99);
    }

    function staticTstore() external {
        assembly {
            tstore(7, 1)
        }
    }

    function internalWrapper(TransientHelper helper) external {
        _innerCall(helper);
    }

    function _innerCall(TransientHelper helper) internal {
        helper.bump(3, 77);
    }
}
"#,
    );
    prj.add_test(
        "TransientDemo.t.sol",
        r#"
// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

import "forge-std/Test.sol";
import "../src/TransientDemo.sol";

contract TransientDemoTest is Test {
    TransientDemo demo;
    TransientHelper helper;

    function setUp() public {
        demo = new TransientDemo();
        helper = new TransientHelper();
    }

    function test_TransientStorageSummary() public {
        uint256 loaded = demo.storeBothAndCompact(address(helper));
        assertEq(loaded, 30);
    }

    function test_StaticCallTstoreHaltsWithoutChange() public view {
        (bool ok,) = address(demo).staticcall(abi.encodeCall(TransientDemo.staticTstore, ()));
        assertFalse(ok);
    }

    function test_DecodeInternalWithTransient() public {
        demo.internalWrapper(helper);
    }

    function test_PauseTracingHidesTransient() public {
        vm.pauseTracing();
        assembly {
            tstore(8, 123)
        }
        helper.bump(8, 123);
        vm.resumeTracing();
        helper.bump(9, 456);
    }
}
"#,
    );

    // -vvvvv shows persistent and transient storage changes, compaction, delegatecall had_value,
    // constructor tstore, and omits net-zero reentrancy guard writes.
    cmd.forge_fuse()
        .args(["test", "--mt", "test_TransientStorageSummary", "-vvvvv", "--no-dynamic-test-linking"])
        .assert_success()
        .stdout_eq(str![[r#"
...
Ran 1 test for test/TransientDemo.t.sol:TransientDemoTest
[PASS] test_TransientStorageSummary() ([GAS])
Traces:
  [..] TransientDemoTest::setUp()
...
  [..] TransientDemoTest::test_TransientStorageSummary()
    ├─ [..] TransientDemo::storeBothAndCompact(TransientHelper: [0x2e234DAe75C793f67A35089C9d99245E1C58470b])
    │   ├─ [..] TransientHelper::bump(2, 40) [delegatecall]
    │   │   ├─  transient storage changes:
    │   │   │   @ 2: 30 → 40
    │   │   └─ ← [Stop]
    │   ├─ [..] → new TransientCtor@[..]
    │   │   ├─  transient storage changes:
    │   │   │   @ 5: 0 → 99
    │   │   └─ ← [Return] [..] bytes of code
    │   ├─  storage changes:
    │   │   @ 0: 0 → 1
    │   ├─  transient storage changes:
    │   │   @ 1: 0 → 20
    │   │   @ 2: 0 → 30
    │   └─ ← [Return] 30
    └─ ← [Stop]

Suite result: ok. 1 passed; 0 failed; 0 skipped; [ELAPSED]

Ran 1 test suite [ELAPSED]: 1 tests passed, 0 failed, 0 skipped (1 total tests)

"#]]);

    // -vvvv does not display transient storage changes.
    cmd.forge_fuse()
        .args(["test", "--mt", "test_TransientStorageSummary", "-vvvv", "--no-dynamic-test-linking"])
        .assert_success()
        .stdout_eq(str![[r#"
...
Ran 1 test for test/TransientDemo.t.sol:TransientDemoTest
[PASS] test_TransientStorageSummary() ([GAS])
Traces:
  [..] TransientDemoTest::test_TransientStorageSummary()
    ├─ [..] TransientDemo::storeBothAndCompact(TransientHelper: [0x2e234DAe75C793f67A35089C9d99245E1C58470b])
    │   ├─ [..] TransientHelper::bump(2, 40) [delegatecall]
    │   │   └─ ← [Stop]
    │   ├─ [..] → new TransientCtor@[..]
    │   │   └─ ← [Return] [..] bytes of code
    │   └─ ← [Return] 30
    └─ ← [Stop]

Suite result: ok. 1 passed; 0 failed; 0 skipped; [ELAPSED]

Ran 1 test suite [ELAPSED]: 1 tests passed, 0 failed, 0 skipped (1 total tests)

"#]]);

    // --opcodes TSTORE,TLOAD emits inline decoded lines (including net-zero lock/unlock) and
    // deduplicates the transient storage changes summary.
    cmd.forge_fuse()
        .args([
            "test",
            "--mt",
            "test_TransientStorageSummary",
            "-vvvvv",
            "--opcodes",
            "TSTORE,TLOAD",
            "--no-dynamic-test-linking",
        ])
        .assert_success()
        .stdout_eq(str![[r#"
...
Ran 1 test for test/TransientDemo.t.sol:TransientDemoTest
[PASS] test_TransientStorageSummary() ([GAS])
Traces:
  [..] TransientDemoTest::setUp()
...
  [..] TransientDemoTest::test_TransientStorageSummary()
    ├─ [..] TransientDemo::storeBothAndCompact(TransientHelper: [0x2e234DAe75C793f67A35089C9d99245E1C58470b])
    │   ├─ [100] TSTORE 0x0: 0x0 → 0x1
    │   ├─ [100] TSTORE 0x2: 0x0 → 0xa
    │   ├─ [100] TSTORE 0x1: 0x0 → 0x14
    │   ├─ [100] TSTORE 0x2: 0xa → 0x1e
    │   ├─ [100] TLOAD 0x2 → (0x1e)
    │   ├─ [100] TSTORE 0x0: 0x1 → 0x0
    │   ├─ [..] TransientHelper::bump(2, 40) [delegatecall]
    │   │   ├─ [100] TSTORE 0x2: 0x1e → 0x28
    │   │   └─ ← [Stop]
    │   ├─ [..] → new TransientCtor@[..]
    │   │   ├─ [100] TSTORE 0x5: 0x0 → 0x63
    │   │   └─ ← [Return] [..] bytes of code
    │   ├─  storage changes:
    │   │   @ 0: 0 → 1
    │   └─ ← [Return] 30
    └─ ← [Stop]

Suite result: ok. 1 passed; 0 failed; 0 skipped; [ELAPSED]

Ran 1 test suite [ELAPSED]: 1 tests passed, 0 failed, 0 skipped (1 total tests)

"#]]);

    // --opcodes TLOAD alone still displays the transient storage changes summary.
    cmd.forge_fuse()
        .args([
            "test",
            "--mt",
            "test_TransientStorageSummary",
            "-vvvvv",
            "--opcodes",
            "TLOAD",
            "--no-dynamic-test-linking",
        ])
        .assert_success()
        .stdout_eq(str![[r#"
...
Ran 1 test for test/TransientDemo.t.sol:TransientDemoTest
[PASS] test_TransientStorageSummary() ([GAS])
Traces:
  [..] TransientDemoTest::setUp()
...
  [..] TransientDemoTest::test_TransientStorageSummary()
    ├─ [..] TransientDemo::storeBothAndCompact(TransientHelper: [0x2e234DAe75C793f67A35089C9d99245E1C58470b])
    │   ├─ [100] TLOAD 0x2 → (0x1e)
    │   ├─ [..] TransientHelper::bump(2, 40) [delegatecall]
    │   │   ├─  transient storage changes:
    │   │   │   @ 2: 30 → 40
    │   │   └─ ← [Stop]
    │   ├─ [..] → new TransientCtor@[..]
    │   │   ├─  transient storage changes:
    │   │   │   @ 5: 0 → 99
    │   │   └─ ← [Return] [..] bytes of code
    │   ├─  storage changes:
    │   │   @ 0: 0 → 1
    │   ├─  transient storage changes:
    │   │   @ 1: 0 → 20
    │   │   @ 2: 0 → 30
    │   └─ ← [Return] 30
    └─ ← [Stop]

Suite result: ok. 1 passed; 0 failed; 0 skipped; [ELAPSED]

Ran 1 test suite [ELAPSED]: 1 tests passed, 0 failed, 0 skipped (1 total tests)

"#]]);

    // A halted TSTORE inside STATICCALL must not emit a transient storage change.
    cmd.forge_fuse()
        .args([
            "test",
            "--mt",
            "test_StaticCallTstoreHaltsWithoutChange",
            "-vvvvv",
            "--no-dynamic-test-linking",
        ])
        .assert_success()
        .stdout_eq(str![[r#"
...
Ran 1 test for test/TransientDemo.t.sol:TransientDemoTest
[PASS] test_StaticCallTstoreHaltsWithoutChange() ([GAS])
Traces:
  [..] TransientDemoTest::setUp()
...
  [..] TransientDemoTest::test_StaticCallTstoreHaltsWithoutChange()
    ├─ [..] TransientDemo::staticTstore() [staticcall]
    │   └─ ← [StateChangeDuringStaticCall] EvmError: StateChangeDuringStaticCall
    └─ ← [Stop]

Suite result: ok. 1 passed; 0 failed; 0 skipped; [ELAPSED]

Ran 1 test suite [ELAPSED]: 1 tests passed, 0 failed, 0 skipped (1 total tests)

"#]]);

    // --decode-internal aligns transient storage changes properly inside nested internal frames.
    cmd.forge_fuse()
        .args([
            "test",
            "--mt",
            "test_DecodeInternalWithTransient",
            "-vvvvv",
            "--decode-internal",
            "--no-dynamic-test-linking",
        ])
        .assert_success()
        .stdout_eq(str![[r#"
...
Ran 1 test for test/TransientDemo.t.sol:TransientDemoTest
[PASS] test_DecodeInternalWithTransient() ([GAS])
Traces:
  [..] TransientDemoTest::setUp()
...
  [..] TransientDemoTest::test_DecodeInternalWithTransient()
    ├─ [..] TransientDemo::internalWrapper(TransientHelper: [0x2e234DAe75C793f67A35089C9d99245E1C58470b])
    │   ├─ [..] TransientDemo::_innerCall(address)(0x2e234DAe75C793f67A35089C9d99245E1C58470b)
    │   │   ├─ [..] TransientHelper::bump(3, 77)
    │   │   │   ├─  transient storage changes:
    │   │   │   │   @ 3: 0 → 77
    │   │   │   └─ ← [Stop]
    │   │   └─ ← 
    │   └─ ← [Stop]
    └─ ← [Stop]

Suite result: ok. 1 passed; 0 failed; 0 skipped; [ELAPSED]

Ran 1 test suite [ELAPSED]: 1 tests passed, 0 failed, 0 skipped (1 total tests)

"#]]);

    // vm.pauseTracing() hides transient writes executed while paused.
    cmd.forge_fuse()
        .args([
            "test",
            "--mt",
            "test_PauseTracingHidesTransient",
            "-vvvvv",
            "--no-dynamic-test-linking",
        ])
        .assert_success()
        .stdout_eq(str![[r#"
...
Ran 1 test for test/TransientDemo.t.sol:TransientDemoTest
[PASS] test_PauseTracingHidesTransient() ([GAS])
Traces:
  [..] TransientDemoTest::setUp()
...
  [..] TransientDemoTest::test_PauseTracingHidesTransient()
    ├─ [0] VM::pauseTracing() [staticcall]
    │   └─ ← [Return]
    ├─ [0] VM::resumeTracing() [staticcall]
    │   └─ ← [Return]
    ├─ [..] TransientHelper::bump(9, 456)
    │   ├─  transient storage changes:
    │   │   @ 9: 0 → 456
    │   └─ ← [Stop]
    └─ ← [Stop]

Suite result: ok. 1 passed; 0 failed; 0 skipped; [ELAPSED]

Ran 1 test suite [ELAPSED]: 1 tests passed, 0 failed, 0 skipped (1 total tests)

"#]]);

    // JSON traces keep TSTORE/TLOAD step.storage_change null and expose typed transient_storage.
    let output = cmd
        .forge_fuse()
        .args([
            "test",
            "--mt",
            "test_TransientStorageSummary",
            "-vvvvv",
            "--json",
            "--no-dynamic-test-linking",
        ])
        .assert_success();
    let json: serde_json::Value =
        serde_json::from_slice(&output.get_output().stdout).expect("valid JSON output");
    let suite = &json["test/TransientDemo.t.sol:TransientDemoTest"]["test_results"];
    let test_result = &suite["test_TransientStorageSummary()"];
    let exec_trace = test_result["traces"]
        .as_array()
        .unwrap()
        .iter()
        .find(|entry| entry[0] == "Execution")
        .expect("execution trace present");
    let arena_obj = &exec_trace[1];

    // No TSTORE/TLOAD step may be mislabelled in persistent `storage_change`.
    for node in arena_obj["arena"].as_array().unwrap() {
        for step in node["trace"]["steps"].as_array().unwrap() {
            if matches!(step["op"].as_str(), Some("TSTORE" | "TLOAD")) {
                assert!(
                    step["storage_change"].is_null(),
                    "transient opcode must not populate persistent storage_change: {step}"
                );
            }
        }
    }

    let transient = arena_obj["transient_storage"]
        .as_object()
        .expect("transient_storage present in JSON trace");
    assert_eq!(transient.len(), 3, "expected 3 nodes with transient accesses: {transient:?}");
    let reasons: Vec<&str> = transient
        .values()
        .flat_map(|v| v.as_array().unwrap())
        .map(|entry| entry["reason"].as_str().unwrap())
        .collect();
    assert!(reasons.contains(&"TSTORE"));
    assert!(reasons.contains(&"TLOAD"));
}
