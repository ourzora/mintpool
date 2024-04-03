use std::future::Future;
use std::str::FromStr;

use alloy_primitives::Signature;
use alloy_sol_types::SolStruct;

use crate::premints::zora_premint_v2::types::ZoraPremintV2;
use crate::types::Premint;

// create premint v2 rule implementations here

// TODO: is there any rust sugar to make this more concise?
//       as it stands, it's not defined as an async function, so can't use async stuff
pub async fn is_authorized_to_create_premint<T: Premint>(premint: &T) -> eyre::Result<bool> {
    //   * if contract exists, check if the signer is the contract admin
    //   * if contract does not exist, check if the signer is the proposed contract admin
    //   * this logic exists as a function on the premint executor contract
    Ok(true)
}

// * signatureIsValid ( this can be performed entirely offline )
//   * check if the signature is valid
//   * check if the signature is equal to the proposed contract admin

async fn is_valid_signature(premint: &ZoraPremintV2) -> eyre::Result<bool> {
    //   * if contract exists, check if the signer is the contract admin
    //   * if contract does not exist, check if the signer is the proposed contract admin

    let signature = Signature::from_str(premint.signature.as_str())?;

    let domain = premint.eip712_domain();
    let hash = premint.premint.eip712_signing_hash(&domain);
    let signer = signature.recover_address_from_prehash(&hash)?;

    Ok(signer == premint.collection.contractAdmin)
}

#[cfg(test)]
mod test {
    use std::str::FromStr;

    use super::*;

    const PREMINT_JSON: &str = r#"
{
  "collection": {
    "contractAdmin": "0xa771209423284bace9a24a06d166a11196724b53",
    "contractURI": "ipfs://bafkreic4fnavhtymee7makmk7wp257nloh5y5ysc2fcwa5rpg6v6f3jhly",
    "contractName": "Karate sketch"
  },
  "premint": {
    "tokenConfig": {
      "tokenURI": "ipfs://bafkreier5h4a6btu24fsitbjdvpyak7moi6wkp33wlqmx2kfwgpq2lvx4y",
      "maxSupply": 18446744073709551615,
      "maxTokensPerAddress": 0,
      "pricePerToken": 0,
      "mintStart": 1702541688,
      "mintDuration": 2592000,
      "royaltyBPS": 500,
      "fixedPriceMinter": "0x04e2516a2c207e84a1839755675dfd8ef6302f0a",
      "payoutRecipient": "0xa771209423284bace9a24a06d166a11196724b53",
      "createReferral": "0x0000000000000000000000000000000000000000"
    },
    "uid": 2,
    "version": 1,
    "deleted": false
  },
  "collectionAddress": "0x42e108d1ed954b0adbd53ea118ba7614622d10d0",
  "chainId": 7777777,
  "signature": "0x894405d100900e6823385ca881c91d5ca7137a326f0c7d27edfd2907d9669cea55626bbd807a36cea815eceeac6634f45cfec54d7157c35f496b999e7b9451de1c"
}"#;

    #[tokio::test]
    async fn test_is_valid_signature() {
        let premint: ZoraPremintV2 = serde_json::from_str(PREMINT_JSON).unwrap();
        assert!(is_valid_signature(&premint).await.expect("failed to check signature"));
    }
}
