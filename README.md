# Mintpool

## Development

This repo uses `just` for task running. See install here: https://github.com/casey/just
or `cargo install just`

### Getting started

Install dependencies:

```sh
just init
```

Seed db for type checked sqlx:

```shell
just seed
```

### Testing

Integration tests can override the config in chains.json by setting `CHAIN_{}_RPC_WSS`. Ex:

```rust
env::set_var("CHAIN_7777777_RPC_WSS", anvil.ws_endpoint());
```