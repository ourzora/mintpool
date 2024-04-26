# Operating a `mintpool` node

## Building

We provide a docker container you can build

```shell
git clone https://github.com/ourzora/mintpool.git
cd mintpool
docker build -t mintpool .
```

Alternatively, you can build with cargo from source

([Install rust](https://www.rust-lang.org/tools/install))

Install dependencies:

```shell
git clone https://github.com/ourzora/mintpool.git
cd mintpool

cargo install just
just init
just seed
```

Then you can build and run

```shell
cargo build --release

SECRET=use_a_real_secret ./target/release/mintpool
```

## Configuration

`mintpool` uses environment variables for configuration, listed below (see `src/config.rs` in code)

Required Configuration:

```
SECRET: u64 (required)      - Secret used to generate the node's keypair for p2p operations.
                              This serves as the node's identity on the network for reputation and connection.
                              Recommended: 32 byte hex random string (ex: `openssl rand -hex 32`)
```

Optional Configuration:

```
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
SYNC_LOOKBACK_HOURS: u64 (6)                - Number of hours to look back for syncing premints from another node
```

**Recommended Configuration for Production:**

```
DATABASE_URL=sqlite://mintpool.db
PERSIST_STATE=true
```

### Configuring blockchain RPC

`mintpool` comes with public RPC urls for a zora network, base, and ethereum. If you want to use
your own RPC urls, or use a chain that's not in the default `SUPPORTED_CHAIN_IDS` you can set the
following environment variables:

```
export CHAIN_{CHAIN_ID}_RPC_WSS=wss://<your_chain_rpc>
# example
export CHAIN_7777777_RPC_WSS=wss://rpc.zora.energy
```

#### Logging

Logging is controlled via the `RUST_LOG` environment variable. We recommend
setting `export RUST_LOG=info`

## Running

You can run the node with the following command, this will give you a node running with a repl

```shell
cargo build --release
RUST_LOG=info SECRET=use_a_real_secret_please ./target/release/mintpool
```

You now will have a node running on with HTTP api running on `http://localhost:7777` and p2p
on `http://localhost:7778`.
Your node should automatically connect to the default boot nodes and sync the last 1 day worth or
premints. See `docs/API.md` for details on the rest api.
