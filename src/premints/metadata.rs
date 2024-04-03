use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct ERC721Metadata {
    pub name: String,
    pub description: String,
    pub image: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct ERC1155Metadata {
    pub name: String,
    pub description: String,
    pub image: String,
    pub decimals: u64,
    pub properties: Map<String, Value>,
}

enum TokenMetadata {
    ERC721(ERC721Metadata),
    ERC1155(ERC1155Metadata),
}
