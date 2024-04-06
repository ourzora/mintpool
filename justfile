init:
    cargo install sqlx-cli

seed:
    touch dev.db
    sqlx migrate run

update-chain-list:
    curl https://chainid.network/chains.json -s -odata/chains.json

ci: init seed
