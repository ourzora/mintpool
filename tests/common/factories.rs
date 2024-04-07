use alloy_primitives::{Address, U256};
use mintpool::types::SimplePremint;
use rand::{Rng, RngCore};

pub trait Factory<O>
where
    Self: Sized,
    O: Default,
{
    fn build(options: O) -> Self;
    fn build_default() -> Self {
        Self::build(O::default())
    }
}

#[derive(Default)]
pub struct SimplePremintOptions {
    chain_id: Option<u64>,
    sender: Option<Address>,
    media: Option<String>,
    token_id: Option<u64>,
}

impl Factory<SimplePremintOptions> for SimplePremint {
    fn build(options: SimplePremintOptions) -> Self {
        let mut rng = rand::thread_rng();

        Self::new(
            U256::from(options.chain_id.unwrap_or(rng.next_u64())),
            options
                .sender
                .unwrap_or(Address::from(rng.gen::<[u8; 20]>())),
            options.token_id.unwrap_or(rng.next_u64()),
            options
                .media
                .unwrap_or("http://example.com/token".to_string()),
        )
    }
}
