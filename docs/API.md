# API Docs

Mintpool exposes a REST API for interacting with the node. The API is ratelimited by default to
prevent spam requests.
See `src/api/mod.rs` for router definition

## `/` Core API

### `GET /health`

Health check endpoint for liveness.

Example

```
curl http://localhost:7777/health

OK
```

### `GET /summary`

Returns information about the node

Example

```
curl -H "Authorization: abc" http://localhost:7777/summary

{
  "commit_sha": "3d917f1",
  "pkg_version": "0.1.0",
  "active_premint_count": 1,
  "total_premint_count": 1,
  "node_info": {
    "local_peer_id": "12D3KooWCY9tjLzwXeWgYe8smxyAhEj7x1TxGG7fMzDLGwzPLEuC",
    "num_peers": 3,
    "dht_peers": [
      [
        "/dnsaddr/mintpool-1.zora.co/p2p/12D3KooWLUCRp7EFvBRGqhZ3kfZT3BRHoxX3a2erBGY5Nm49ggqy",
        "/dnsaddr/mintpool-1.zora.co"
      ],
      [
        "/dnsaddr/mintpool-3.zora.co/p2p/12D3KooWSgM2s7sJjKt7Tf3eXSDduszS6ZonaY444Yz7sNNVW7K9",
        "/dnsaddr/mintpool-3.zora.co"
      ],
      [
        "/dnsaddr/mintpool-2.zora.co/p2p/12D3KooWEBYjav7N175YYuEsPFdm36vKywjktcaE1HFgMTnQNWmy",
        "/dnsaddr/mintpool-2.zora.co"
      ]
    ],
    "gossipsub_peers": [
      "12D3KooWEBYjav7N175YYuEsPFdm36vKywjktcaE1HFgMTnQNWmy",
      "12D3KooWLUCRp7EFvBRGqhZ3kfZT3BRHoxX3a2erBGY5Nm49ggqy",
      "12D3KooWSgM2s7sJjKt7Tf3eXSDduszS6ZonaY444Yz7sNNVW7K9"
    ],
    "all_external_addresses": [
      [
        "/dnsaddr/mintpool-1.zora.co"
      ],
      [
        "/dnsaddr/mintpool-3.zora.co"
      ],
      [
        "/dnsaddr/mintpool-2.zora.co"
      ]
    ]
  }
}
```

### `GET /list-all`

List all premints stored by the node. Supports the following query params for filtering:

```
chain_id: u64                   -- filter by chain id
kind: String,                   -- filter by kind (zora_v2)
collection_address: Address,    -- filter by collection address a premint belongs to
creator_address: Address,       -- filter by creator's wallet
from: UTC timestamp string,     -- filter by created_at >= from
to: UTC timestamp string,       -- filter by created_at <= to
```

Example

```
curl http://localhost:7777/list-all?chain_id=7777777&kind=zora_v2

[
  {
    "zoraV2": {
      "collection": {
        "contractAdmin": "0xd272a3cb66bea1fa7547dad5b420d5ebe14222e5",
        "contractURI": "ipfs://bafkreicuxlqqgoo6fxlmijqvilckvwj6ey26yvzpwg73ybcltvvek2og6i",
        "contractName": "Fancy title"
      },
      "premint": {
        "tokenConfig": {
          "tokenURI": "ipfs://bafkreia474gkk2ak5eeqstp43nqeiunqkkfeblctna3y54av7bt6uwehmq",
          "maxSupply": "0xffffffffffffffff",
          "maxTokensPerAddress": 0,
          "pricePerToken": 0,
          "mintStart": 1708100240,
          "mintDuration": 2592000,
          "royaltyBPS": 500,
          "payoutRecipient": "0xd272a3cb66bea1fa7547dad5b420d5ebe14222e5",
          "fixedPriceMinter": "0x04e2516a2c207e84a1839755675dfd8ef6302f0a",
          "createReferral": "0x0000000000000000000000000000000000000000"
        },
        "uid": 1,
        "version": 1,
        "deleted": false
      },
      "collectionAddress": "0x0cfbce0e2ea475d6413e2f038b2b62e64106ad1f",
      "chainId": 7777777,
      "signature": "0x2eb4d27a5b04fd41bdd33f66a18a4993c0116724c5fe5b8dc20bf22f45455c621139eabdbd27434e240938a60b1952979c9dc9c8a141cc71764786fe4d3f909f1c"
    }
  }
]

```

### `GET /get-one`

Gets a single premint that matches the query criterial

```
chain_id: u64                   -- filter by chain id
kind: String,                   -- filter by kind (zora_v2)
collection_address: Address,    -- filter by collection address a premint belongs to
creator_address: Address,       -- filter by creator's wallet
from: UTC timestamp string,     -- filter by created_at >= from
to: UTC timestamp string,       -- filter by created_at <= to
```

Example

```
curl http://localhost:7777/get-one?chain_id=7777777&kind=zora_v2&creator_address=0x0cfbce0e2ea475d6413e2f038b2b62e64106ad1f

{
"zoraV2": {
  "collection": {
    "contractAdmin": "0xd272a3cb66bea1fa7547dad5b420d5ebe14222e5",
    "contractURI": "ipfs://bafkreicuxlqqgoo6fxlmijqvilckvwj6ey26yvzpwg73ybcltvvek2og6i",
    "contractName": "Fancy title"
  },
  "premint": {
    "tokenConfig": {
      "tokenURI": "ipfs://bafkreia474gkk2ak5eeqstp43nqeiunqkkfeblctna3y54av7bt6uwehmq",
      "maxSupply": "0xffffffffffffffff",
      "maxTokensPerAddress": 0,
      "pricePerToken": 0,
      "mintStart": 1708100240,
      "mintDuration": 2592000,
      "royaltyBPS": 500,
      "payoutRecipient": "0xd272a3cb66bea1fa7547dad5b420d5ebe14222e5",
      "fixedPriceMinter": "0x04e2516a2c207e84a1839755675dfd8ef6302f0a",
      "createReferral": "0x0000000000000000000000000000000000000000"
    },
    "uid": 1,
    "version": 1,
    "deleted": false
  },
  "collectionAddress": "0x0cfbce0e2ea475d6413e2f038b2b62e64106ad1f",
  "chainId": 7777777,
  "signature": "0x2eb4d27a5b04fd41bdd33f66a18a4993c0116724c5fe5b8dc20bf22f45455c621139eabdbd27434e240938a60b1952979c9dc9c8a141cc71764786fe4d3f909f1c"
}
}

```

### `GET /get-one/{kind}/{id}`

Gets a single premint that matches the path query for kind and id

Response: `PremintType`

Example

```
curl http://localhost:7777/get-one/zora_v2/7777777:0x0cfbce0e2ea475d6413e2f038b2b62e64106ad1f:1
{
"zoraV2": {
  "collection": {
    "contractAdmin": "0xd272a3cb66bea1fa7547dad5b420d5ebe14222e5",
    "contractURI": "ipfs://bafkreicuxlqqgoo6fxlmijqvilckvwj6ey26yvzpwg73ybcltvvek2og6i",
    "contractName": "Fancy title"
  },
  "premint": {
    "tokenConfig": {
      "tokenURI": "ipfs://bafkreia474gkk2ak5eeqstp43nqeiunqkkfeblctna3y54av7bt6uwehmq",
      "maxSupply": "0xffffffffffffffff",
      "maxTokensPerAddress": 0,
      "pricePerToken": 0,
      "mintStart": 1708100240,
      "mintDuration": 2592000,
      "royaltyBPS": 500,
      "payoutRecipient": "0xd272a3cb66bea1fa7547dad5b420d5ebe14222e5",
      "fixedPriceMinter": "0x04e2516a2c207e84a1839755675dfd8ef6302f0a",
      "createReferral": "0x0000000000000000000000000000000000000000"
    },
    "uid": 1,
    "version": 1,
    "deleted": false
  },
  "collectionAddress": "0x0cfbce0e2ea475d6413e2f038b2b62e64106ad1f",
  "chainId": 7777777,
  "signature": "0x2eb4d27a5b04fd41bdd33f66a18a4993c0116724c5fe5b8dc20bf22f45455c621139eabdbd27434e240938a60b1952979c9dc9c8a141cc71764786fe4d3f909f1c"
}
}
```

### `POST /submit-premint`

Submit a premint to the node. The node will store the premint if passes all rules, then broadcast it
to peer nodes.

Example

```
curl -X POST http://localhost:7777/submit-premint -H "Content-Type: application/json" -d '{
  "zoraV2": {
    "collection": {
      "contractAdmin": "0xd272a3cb66bea1fa7547dad5b420d5ebe14222e5",
      "contractURI": "ipfs://bafkreicuxlqqgoo6fxlmijqvilckvwj6ey26yvzpwg73ybcltvvek2og6i",
      "contractName": "Fancy title"
    },
    "premint": {
      "tokenConfig": {
        "tokenURI": "ipfs://bafkreia474gkk2ak5eeqstp43nqeiunqkkfeblctna3y54av7bt6uwehmq",
        "maxSupply": "18446744073709551615",
        "maxTokensPerAddress": 0,
        "pricePerToken": 0,
        "mintStart": 1708100240,
        "mintDuration": 2592000,
        "royaltyBPS": 500,
        "fixedPriceMinter": "0x04e2516a2c207e84a1839755675dfd8ef6302f0a",
        "payoutRecipient": "0xd272a3cb66bea1fa7547dad5b420d5ebe14222e5",
        "createReferral": "0x0000000000000000000000000000000000000000"
      },
      "uid": 1,
      "version": 1,
      "deleted": false
    },
    "collectionAddress": "0x0cfbce0e2ea475d6413e2f038b2b62e64106ad1f",
    "chainId": 7777777,
    "signer": "0xd272a3cb66bea1fa7547dad5b420d5ebe14222e5",
    "signature": "0x2eb4d27a5b04fd41bdd33f66a18a4993c0116724c5fe5b8dc20bf22f45455c621139eabdbd27434e240938a60b1952979c9dc9c8a141cc71764786fe4d3f909f1c"
  }
}'

# Success
{
  "Success": {
    "message": "Premint submitted"
  }
}

# Error
{
  "Error": {
    "message": "Premint not submitted"
  }
}

# Rejected
{
  "RulesError": {
    "evaluation": [
      {
        "result": "accept",
        "reason": null,
        "rule_name": "Metadata::token_uri_length"
      },
      {
        "result": "accept",
        "reason": null,
        "rule_name": "Metadata::existing_token_uri"
      },
      {
        "result": "accept",
        "reason": null,
        "rule_name": "Metadata::signer_matches"
      },
      {
        "result": "reject",
        "reason": "Existing premint with higher version 1 exists",
        "rule_name": "Metadata::version_is_higher"
      },
      {
        "result": "accept",
        "reason": null,
        "rule_name": "PremintTypes::ZoraV2::is_authorized_to_create_premint"
      },
      {
        "result": "accept",
        "reason": null,
        "rule_name": "PremintTypes::ZoraV2::is_valid_signature"
      },
      {
        "result": "accept",
        "reason": null,
        "rule_name": "PremintTypes::ZoraV2::is_chain_supported"
      },
      {
        "result": "accept",
        "reason": null,
        "rule_name": "PremintTypes::ZoraV2::not_minted"
      },
      {
        "result": "accept",
        "reason": null,
        "rule_name": "PremintTypes::ZoraV2::premint_version_supported"
      }
    ]
  }
}

```

## `/admin` Admin API

Routes for admin actions, all subroutes are gated by an admin key (see `docs/OPERATION.md`).
If the admin key is not set these routes are unreachable.

### `POST /admin/submit-premint`

See `/submit-premint` for details, same route but without ratelimit

### `POST /admin/add-peer`

Sends the node a peer address to connect to

Example

```
curl -X POST http://localhost:7777/admin/add-peer -H "Content-Type: application/json" -H "Authorization: abc" -d '{
    "peer": "/ip4/103.106.59.158/tcp/7778/p2p/12D3KooWLhb58g62Q9pLxAF7Ux7gPwgEXcHQrQY8MRxPv8qnd5SH"
}'

{"Success":{"message":"Peer added"}}
```

### `POST /admin/sync`

Starts a sync against a random node

```
curl -X POST http://localhost:7777/admin/async -H "Authorization: abc"

200
```

## `/metrics` Prometheus Metrics scrape endpoint

Returns prometheus metrics for the node
