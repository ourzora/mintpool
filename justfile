init:
    cargo install sqlx-cli

seed:
    touch dev.db
    sqlx migrate run

ci: init seed
