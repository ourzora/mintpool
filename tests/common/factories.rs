use alloy::primitives::Address;
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
    version: Option<u64>,
    chain_id: Option<u64>,
    sender: Option<Address>,
    media: Option<String>,
    token_id: Option<u64>,
}

impl Factory<SimplePremintOptions> for SimplePremint {
    fn build(options: SimplePremintOptions) -> Self {
        let mut rng = rand::thread_rng();

        Self::new(
            options.version.unwrap_or(1),
            options
                .chain_id
                .unwrap_or(rng.gen_range(1..=i64::MAX as u64)),
            options
                .sender
                .unwrap_or(Address::from(rng.gen::<[u8; 20]>())),
            options.token_id.unwrap_or(rng.next_u64()),
            options
                .media
                .unwrap_or("https://example.com/token".to_string()),
        )
    }
}
