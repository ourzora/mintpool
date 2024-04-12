init:
    cargo install sqlx-cli

seed:
    touch dev.db
    sqlx migrate run

update-chain-list:
    curl https://chainid.network/chains.json -s -o- | json_pp > data/chains.json


ci: init seed
