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
    bytes32 salt = 0x000000000000000000000000000000000000000004debe9873087003b05925fc;
    address expectedAddress = 0x777777bAE7A2628598AB9FAcc367f42C957E06a7;

    ImmutableCreate2Factory constant IMMUTABLE_CREATE2_FACTORY =
        ImmutableCreate2Factory(0x0000000000FFe8B47B3e2130213B802212439497);

    function setUp() public {}

    function run() public {
        string[] memory trustedNodes = new string[](2);
        trustedNodes[0] = "/connect/dns/mintpool.zora.co/tcp/7778";
        // TODO: Add nodes here once DNS is set up

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
