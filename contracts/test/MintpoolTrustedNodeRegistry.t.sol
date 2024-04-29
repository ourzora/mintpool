// SPDX-License-Identifier: UNLICENSED
pragma solidity ^0.8.13;

import {Test, console} from "forge-std/Test.sol";
import {MintpoolTrustedNodeRegistry} from "../src/MintpoolTrustedNodeRegistry.sol";

contract MintpoolTrustedNodeRegistryTest is Test {
    MintpoolTrustedNodeRegistry registry;
    address zora = 0xd1d1D4e36117aB794ec5d4c78cBD3a8904E691D0;

    string node1 = "/connect/dns/mintpool.zora.co/tcp/7778";
    string node2 = "/ip4/127.0.0.1/tcp/7779/p2p/12D3KooWQYhTNQdmr3ArTeUHRYzFg94BKyTkoWBDWez9kSCVe2Xo";


    function setUp() public {
        string[] memory trustedNodes = new string[](2);
        trustedNodes[0] = node1;
        trustedNodes[1] = node2;
        registry = new MintpoolTrustedNodeRegistry();
        registry.initialize(trustedNodes, zora);

        console.logAddress(registry.owner());
    }

    function test_addTrustedNode() public {
        uint256 trustedNodeCount = registry.trustedNodeCount();
        assertEq(trustedNodeCount, 2);

        vm.prank(zora);
        registry.addTrustedNode("node3");

        bool trusted = registry.isTrustedNode("node3");
        assertEq(trusted, true);
        trustedNodeCount = registry.trustedNodeCount();
        assertEq(trustedNodeCount, 3);
    }

    function test_removeTrustedNode() public {
        bool trusted = registry.isTrustedNode(node1);
        uint256 trustedNodeCount = registry.trustedNodeCount();
        assertEq(trustedNodeCount, 2);
        assertEq(trusted, true);

        vm.prank(zora);
        registry.removeTrustedNode(node1);

        trusted = registry.isTrustedNode(node1);
        assertEq(trusted, false);
        trustedNodeCount = registry.trustedNodeCount();
        assertEq(trustedNodeCount, 1);
    }

    function test_removeTrustedNode_onlyOwner() public {
        vm.expectRevert("UNAUTHORIZED");
        registry.removeTrustedNode(node1);
    }

    function test_isTrustedNode() public view {
        bool trusted = registry.isTrustedNode(node1);
        assertEq(trusted, true);

        string[] memory shouldBeTrusted = new string[](2);
        shouldBeTrusted[0] = node1;
        shouldBeTrusted[1] = node2;
        bool[] memory trustedNodes = registry.isTrustedNode(shouldBeTrusted);
        for (uint256 i = 0; i < trustedNodes.length; i++) {
            assertEq(trustedNodes[i], true);
        }

        trusted = registry.isTrustedNode("foo");
        assertEq(trusted, false);

        string[] memory shouldNotBeTrusted = new string[](2);
        shouldNotBeTrusted[0] = "node1";
        shouldNotBeTrusted[1] = "node2";
        trustedNodes = registry.isTrustedNode(shouldNotBeTrusted);
        for (uint256 i = 0; i < trustedNodes.length; i++) {
            assertEq(trustedNodes[i], false);
        }
    }

    function test_addRemoveTrustedNodeBatch() public {
        string[] memory nodes = new string[](2);
        nodes[0] = "node3";
        nodes[1] = "node4";
        vm.prank(zora);
        registry.addTrustedNodeBatch(nodes);

        bool trusted = registry.isTrustedNode("node3");
        assertEq(trusted, true);
        trusted = registry.isTrustedNode("node4");
        assertEq(trusted, true);

        vm.prank(zora);
        registry.removeTrustedNodeBatch(nodes);
        trusted = registry.isTrustedNode("node3");
        assertEq(trusted, false);
        trusted = registry.isTrustedNode("node4");
        assertEq(trusted, false);
    }

    function test_addTrustedNodeBatch_onlyOwner() public {
        string[] memory nodes = new string[](2);
        nodes[0] = "node3";
        nodes[1] = "node4";
        vm.expectRevert("UNAUTHORIZED");
        registry.addTrustedNodeBatch(nodes);
    }

    function test_removeTrustedNodeBatch_onlyOwner() public {
        string[] memory nodes = new string[](2);
        nodes[0] = "node3";
        nodes[1] = "node4";
        vm.expectRevert("UNAUTHORIZED");
        registry.removeTrustedNodeBatch(nodes);
    }
}
