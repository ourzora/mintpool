// SPDX-License-Identifier: UNLICENSED
pragma solidity ^0.8.13;

import "solady/auth/Ownable.sol";

contract MintpoolTrustedNodeRegistry is Ownable {
    string[] _trustedNodes;

    constructor(string[] memory trustedNodes, address _owner) {
        _trustedNodes = trustedNodes;
        _initializeOwner(_owner);
        _setOwner(_owner);
    }

    function addTrustedNode(string memory _node) public onlyOwner {
        _trustedNodes.push(_node);
    }

    function removeTrustedNode(string memory _node) public onlyOwner {
        uint256 last = _trustedNodes.length - 1;
        string memory lastNode = _trustedNodes[last];
        bytes32 nodeBytes = keccak256(abi.encode(_node));

        for (uint256 i = 0; i < _trustedNodes.length; i++) {
            if (keccak256(abi.encode(_trustedNodes[i])) == nodeBytes) {
                _trustedNodes[last] = _trustedNodes[i];
                _trustedNodes[i] = lastNode;
                _trustedNodes.pop();
                break;
            }
        }
    }

    function listTrustedNodes() public view returns (string[] memory) {
        return _trustedNodes;
    }
}
