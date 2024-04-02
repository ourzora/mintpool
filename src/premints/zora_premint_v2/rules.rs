// create premint v2 rule implementations here

// * isAuthorizedToCreatePremint ( this exists as an rpc call on the preminter contract )
//   * if contract exists, check if the signer is the contract admin
//   * if contract does not exist, check if the signer is the proposed contract admin

// * signatureIsValid ( this can be performed entirely offline )
//   * check if the signature is valid
//   * check if the signature is equal to the proposed contract admin
