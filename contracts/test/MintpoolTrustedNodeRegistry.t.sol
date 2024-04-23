// SPDX-License-Identifier: UNLICENSED
pragma solidity ^0.8.13;

import {Test, console} from "forge-std/Test.sol";
import {MintpoolTrustedNodeRegistry} from "../src/MintpoolTrustedNodeRegistry.sol";

contract CounterTest is Test {
    MintpoolTrustedNodeRegistry registry;

    function setUp() public {
        string[] memory trustedNodes = new string[](2);
        trustedNodes[0] = "node1";
        trustedNodes[1] = "node2";
        registry = new MintpoolTrustedNodeRegistry(trustedNodes, msg.sender);

        console.logAddress(registry.owner());
    }

    function test_listTrustedNodes() public {
        console.logAddress(registry.owner());
        string[] memory nodes = registry.listTrustedNodes();
        assertEq(nodes.length, 2);
        assertEq(nodes[0], "node1");
        assertEq(nodes[1], "node2");
    }

    function test_addTrustedNode() public {
        console.logAddress(registry.owner());
        registry.addTrustedNode("node3");
        string[] memory nodes = registry.listTrustedNodes();
        assertEq(nodes.length, 3);
        assertEq(nodes[2], "node3");
    }

    function test_removeTrustedNode() public {
        console.logAddress(registry.owner());
        string[] memory nodes = registry.listTrustedNodes();
        assertEq(nodes.length, 2);
        assertEq(nodes[0], "node1");
        assertEq(nodes[1], "node2");

        registry.removeTrustedNode("node1");

        nodes = registry.listTrustedNodes();
        assertEq(nodes.length, 1);
        assertEq(nodes[0], "node2");
    }
}
