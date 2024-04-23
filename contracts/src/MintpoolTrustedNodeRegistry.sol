// SPDX-License-Identifier: UNLICENSED
pragma solidity ^0.8.13;

import "./Owned.sol";

/**
 * @title MintpoolTrustedNodeRegistry
 * @dev A contract to manage a list of trusted nodes
 */
contract MintpoolTrustedNodeRegistry is Owned {
    mapping(bytes32 => bool) _trustedNodeMap;
    uint256 public trustedNodeCount;
    bool initialized;

    event TrustedNodeAdded(string node);
    event TrustedNodeRemoved(string node);


    function initialize(string[] memory trustedNodes, address _owner) public {
        require(trustedNodeCount == 0, "ALREADY_INITIALIZED");
        for (uint256 i = 0; i < trustedNodes.length; i++) {
            _addTrustedNode(trustedNodes[i]);
        }
        initOwner(_owner);
    }

    /**
     * @dev Add a trusted node
     * @param _node The node to add
     */
    function addTrustedNode(string memory _node) public onlyOwner {
        _addTrustedNode(_node);
    }

    /**
     * @dev Check if a node is trusted
     * @param _node The node to check
     * @return bool
     */
    function isTrustedNode(string memory _node) public view returns (bool) {
        return _trustedNodeMap[hashNode(_node)];
    }

    /**
     * @dev Check if a list of nodes are trusted
     * @param _nodes The nodes to check
     * @return bool[] An array of booleans indicating if the node is trusted
     */
    function isTrustedNode(string[] memory _nodes) public view returns (bool[] memory) {
        bool[] memory results = new bool[](_nodes.length);
        for (uint256 i = 0; i < _nodes.length; i++) {
            results[i] = isTrustedNode(_nodes[i]);
        }
        return results;
    }

    /**
     * @dev Remove a trusted node
     * @param _node The node to remove
     */
    function removeTrustedNode(string memory _node) public onlyOwner {
        bytes32 nodeHash = hashNode(_node);
        _trustedNodeMap[nodeHash] = false;
        trustedNodeCount--;
        emit TrustedNodeRemoved(_node);
    }

    function hashNode(string memory _node) internal pure returns (bytes32) {
        return keccak256(abi.encode(_node));
    }

    function _addTrustedNode(string memory _node) private {
        bytes32 nodeHash = keccak256(abi.encode(_node));

        require(!_trustedNodeMap[nodeHash], "NODE_ALREADY_TRUSTED");

        _trustedNodeMap[nodeHash] = true;
        trustedNodeCount++;
        emit TrustedNodeAdded(_node);
    }
}
