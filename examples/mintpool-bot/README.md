# Mintpool bot example

A simple, not optimized, example of how to use the rules engine of mintpool to create a bot that
mints premints from a specific creator.

To run

```
export SQLX_OFFLINE=true        # if not set, sqlx checks queries against the db, disable that since no db
# if you don't set `CREATOR_ADDRESS` it'll default to jacob.eth        
PRIVATE_KEY=<0xverysecret> SECRET=<your node secret> CREATOR_ADDRESS=0xAbC...123 cargo run --release
```

