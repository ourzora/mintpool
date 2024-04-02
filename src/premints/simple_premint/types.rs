use alloy_sol_macro::sol;
use serde::{Deserialize, Serialize};

sol! {
    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    struct SimplePremint {
        uint64 chain_id;
        address sender;
        uint64 token_id;
        string name;
        string description;
        string media;
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test() {
        let premint = SimplePremint {
            chain_id: 1,
            sender: "0x7e5A9B6F4bB9efC27F83E18F29e4326480668f87".parse().unwrap(),
            media: "ipfs://tokenIpfsId0".parse().unwrap(),
            token_id: 1,
        };

        println!("{:?}", premint);
    }
}