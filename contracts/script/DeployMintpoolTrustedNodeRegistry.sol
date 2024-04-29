// SPDX-License-Identifier: UNLICENSED
pragma solidity ^0.8.13;

import {Script, console} from "forge-std/Script.sol";
import {MintpoolTrustedNodeRegistry} from "../src/MintpoolTrustedNodeRegistry.sol";

interface ImmutableCreate2Factory {
    function findCreate2Address(bytes32 salt, bytes memory initCode)
    external
    view
    returns (address deploymentAddress);

    function findCreate2AddressViaHash(bytes32 salt, bytes32 initCodeHash)
    external
    view
    returns (address deploymentAddress);

    function hasBeenDeployed(address deploymentAddress) external view returns (bool);

    function safeCreate2(bytes32 salt, bytes memory initializationCode)
    external
    payable
    returns (address deploymentAddress);
}

contract DeployScript is Script {
    // Generated using https://github.com/iainnash/create2crunch/tree/use_prefix_matching_instead
    address zora = 0xd1d1D4e36117aB794ec5d4c78cBD3a8904E691D0;
    bytes32 salt = 0x00000000000000000000000000000000000000000a54fd9f3c211d018424dd4e;
    address expectedAddress = 0x7777770105719d36De7E5A0a26536D6482234Ccd;

    ImmutableCreate2Factory constant IMMUTABLE_CREATE2_FACTORY =
    ImmutableCreate2Factory(0x0000000000FFe8B47B3e2130213B802212439497);

    function setUp() public {}

    function run() public {
        string[] memory trustedNodes = new string[](3);
        trustedNodes[0] = "/dnsaddr/mintpool-1.zora.co/p2p/12D3KooWLUCRp7EFvBRGqhZ3kfZT3BRHoxX3a2erBGY5Nm49ggqy";
        trustedNodes[1] = "/dnsaddr/mintpool-2.zora.co/p2p/12D3KooWEBYjav7N175YYuEsPFdm36vKywjktcaE1HFgMTnQNWmy";
        trustedNodes[2] = "/dnsaddr/mintpool-3.zora.co/p2p/12D3KooWSgM2s7sJjKt7Tf3eXSDduszS6ZonaY444Yz7sNNVW7K9";

        bytes memory creationCode = type(MintpoolTrustedNodeRegistry).creationCode;
        bytes32 creationCodeHash = keccak256(creationCode);
        console.logBytes32(creationCodeHash);

        require(IMMUTABLE_CREATE2_FACTORY.findCreate2Address(salt, creationCode) == expectedAddress, "Address mismatch");

        vm.startBroadcast();
        address deployedAddress = IMMUTABLE_CREATE2_FACTORY.safeCreate2(salt, creationCode);
        require(deployedAddress == expectedAddress, "address different than expected");
        require(IMMUTABLE_CREATE2_FACTORY.hasBeenDeployed(expectedAddress), "Not deployed");

        MintpoolTrustedNodeRegistry(deployedAddress).initialize(trustedNodes, zora);
        vm.stopBroadcast();
    }
}
