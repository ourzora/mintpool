![](data/mintpool.png)

# Mintpool

A decentralized mempool for prechain transactions.

## What?

`mintpool` is a decentralized, mempool like system for things that can be brought onchain. It
uses `libp2p` to allow connected nodes to communicate messages to each other, and exposes
that data via rest apis. `mintpool` features a custom rules engine that allows each node to include
or exclude premints they want to store in their local node, based on arbitrary criteria. `mintpool`
will also prune premints that end up being brought onchain, keeping the data served by the node
limited to things that can be brought onchain.

## Why?

Transactions deserve to be onchain, but paying gas is tricky ux to nail for non-crypto native folks,
and at times can be expensive.

`mintpool` solves this by creating a mempool of transactions others can sponsor to bring onchain
for a reward, starting with Zora's Premints.

When you bring a Zora premint onchain you pay the gas for the transaction, but in exchange you
receive the `first minter` portion of the mint fee for every mint of that NFT going
forward (`0.000111 ETH` per mint).

## What can I do with mintpool?

### Create without worrying about gas

Submitting a premint to a mintpool node is just a `POST` request away, making it easy to build gas
free creation experiences, while getting paid as a platform.

Platforms that mint using zora's protocol can set themselves as the `createReferral`, and receive
the `creator referral` portion of the mint fee for every mint from that creator going
forward (`0.000111 ETH` per mint).

TODO: add instructions on creating a premint here

```
POST /submit-premint
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
}    
```

### Permissionless aggregator experiences

You can show Zora premint NFTs without having to trust the `zora.co` api or write and indexer
yourself.
Each `mintpool` node stores premints based on their nodes rules, and should see each message sent
through the p2p network. This means your node can be your source of truth for the premint NFTs you
care about. Each premint contains all the info needed to display and bring an NFT onchain.

TODO: example API requests

### Extend with custom rules and endpoints

`mintpool` can be used as a binary or as a library, allowing you to extend the functionality of your
node.
Only want to store premints from a certain creator? You can do that. Don't like our rest api? You
can add a graphql api by mounting routes on to our `axum` router.

See `examples/` for code examples on how to add custom rules and endpoints.

### MEV Bots (Mint Extractable Value Bots)

Bots are good, they bring onchain liquidity to people who don't want to pay gas and can be lucrative
if done correctly.

`mintpool` nodes can execute arbitrary rules, meaning you could write a rule that mints a premint,
turning your node into a bot running on the p2p network.

## Operating a node

See [docs/OPERATION.md](docs/OPERATION.md) for more information on how to operate a `mintpool` node.

## Contributing

See `docs/DEVELOPMENT.md` for more information on how to develop `mintpool`.

Zora will accept PRs that expand the functionality of `mintpool`, including supporting other
protocols.
