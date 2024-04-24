# Operating a `mintpool` node

## Building

We provide a docker container you can build

```shell
git clone https://github.com/ourzora/mintpool.git
cd mintpool
docker build -t mintpool .
```

Alternatively, you can build with cargo from source

```shell
git clone https://github.com/ourzora/mintpool.git
cd mintpool
cargo build --release

./target/release/mintpool
```

or use `cargo install`

```shell
cargo install --git https://github.com/ourzora/mintpool.git
mintpool
```

## Configuration

`mintpool` uses environment variables for configuration, listed below (see `src/config.rs` in code)

All configuration is optional, and defaults are provided for everything other than `SEED`

```
SEED: u64 (required)                        - Seed for peer id
PEER_PORT: u64 (7778)                       - Port to listen for p2p connections from other nodes
CONNECT_EXTERNAL: bool (true)               - If true, the node will run on 0.0.0.0 instead of 127.0.0.1
DATABASE_URL: String ("sqlite::memory:")    - sqlite connection string (ex: sqlite://mintpool.db, sqlite::memory:)
PERSIST_STATE: bool (false)                 - If true, the node will persist state to the database, otherwise it will only store in memory.
                                                if set to true, DATABASE_URL is required
PRUNE_MINTED_PREMINTS: bool (true)          - If true, the node will delete minted premints from the database, 
                                                if false it will just mark as `seen_on_chain` in the db but not delete 
API_PORT: u64 (7777)                        - Port to listen for http api requests
PEER_LIMIT: u64 (1000)                      - Maximum number of peers to connect to
PREMINT_TYPES: String ("zora_premint_v2")   - Comma separated list of default premint types to process
CHAIN_INCLUSION_MODE: String ("verify")     - Chain inclusion mode, either `check`, `verify`, or `ignore` (see below)
SUPPORTED_CHAIN_IDS: String ("7777777,8453")- Comma separated list of chain ids to support
TRUSTED_PEERS: Option<String> (None)        - Comma separated list of peers to trust when they send notice of seeing a premint onchain
NODE_ID: Option<u64> (None)                 - Node id for logging purposes
EXTERNAL_ADDRESS: Option<String> (None)     - External address for the node for logging purposes
INTERACTIVE: bool (false)                   - If true, interactive repl will run with node so you can interact from your terminal
ENABLE_RPC: bool (true)                     - If true, rpc will be used for rules evaluation
ADMIN_API_SECRET: Option<String> (None)     - Secret key used to access admin api routes
RATE_LIMIT_RPS: u32 (2)                     - Rate limit requests per second for the http api
```

#### Logging

Logging is controlled via the `RUST_LOG` environment variable. We recommend
setting `export RUST_LOG=info`

## Running locally

You can run the node with the following command, this will give you a node running with a repl

```shell
RUST_LOG=info SEED=1 INTERACTIVE=true mintpool
```
