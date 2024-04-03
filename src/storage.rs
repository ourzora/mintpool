use crate::config::Config;
use crate::types::{Premint, PremintTypes};
use eyre::WrapErr;
use sqlx::{Row, SqlitePool};

async fn init_db(config: &Config) -> SqlitePool {
    let expect_msg =
        "Failed to connect to DB. Ensure envar DATABASE_URL is set or ensure PERSIST_STATE=false.";

    if config.persist_state {
        SqlitePool::connect(&config.db_url.clone().expect(expect_msg))
            .await
            .expect(expect_msg)
    } else {
        SqlitePool::connect("sqlite::memory:")
            .await
            .expect("Failed to connect to in-memory DB. This should never happen.")
    }
}

pub struct PremintStorage {
    db: SqlitePool,
    prune_minted_premints: bool,
}

impl PremintStorage {
    pub async fn new(config: &Config) -> Self {
        let db = init_db(config).await;
        Self::create_premint_table(&db)
            .await
            .expect("Failed to create premint table");
        Self {
            db,
            prune_minted_premints: config.prune_minted_premints,
        }
    }

    async fn create_premint_table(db: &SqlitePool) -> eyre::Result<()> {
        sqlx::migrate!("./migrations")
            .run(db)
            .await
            .wrap_err("Failed to run migrations")?;
        Ok(())
    }

    pub fn db(&self) -> SqlitePool {
        self.db.clone()
    }

    pub async fn store(&self, premint: PremintTypes) -> eyre::Result<()> {
        let metadata = premint.metadata();
        let json = premint.to_json()?;
        let signer = format!("{:?}", metadata.signer);
        let collection_address = format!("{:?}", metadata.collection_address);
        let token_id = metadata.token_id.to_string();
        let chain_id = metadata.chain_id.to::<i64>();
        let id = premint.guid();
        sqlx::query!(
            r#"
            INSERT INTO premints (id, kind, signer, chain_id, collection_address, token_id, json)
            VALUES (?, ?, ?, ?, ?, ?, ?)
        "#,
            id,
            metadata.kind.0,
            signer,
            chain_id,
            collection_address,
            token_id,
            json
        )
        .execute(&self.db)
        .await
        .map_err(|e| eyre::eyre!("Failed to store premint: {}", e))?;
        Ok(())
    }

    pub async fn list_all(&self) -> eyre::Result<Vec<PremintTypes>> {
        let rows = sqlx::query(
            r#"
            SELECT json FROM premints
        "#,
        )
        .fetch_all(&self.db)
        .await
        .map_err(|e| eyre::eyre!("Failed to list all premints: {}", e))?;
        let premints = rows
            .iter()
            .map(|row| {
                let json: String = row.get(0);
                PremintTypes::from_json(json).unwrap()
            })
            .collect();
        Ok(premints)
    }

    pub async fn get_for_id(&self, id: String) -> eyre::Result<PremintTypes> {
        let row = sqlx::query(
            r#"
            SELECT json FROM premints WHERE id = ?
        "#,
        )
        .bind(id)
        .fetch_one(&self.db)
        .await?;
        let json = row.get(0);
        PremintTypes::from_json(json)
    }
}

#[cfg(test)]
mod test {
    use crate::config::{ChainInclusionMode, Config};
    use crate::storage::PremintStorage;
    use crate::types::{Premint, PremintTypes};

    #[tokio::test]
    async fn test_insert_and_get() {
        let config = Config {
            seed: 0,
            port: 7777,
            connect_external: false,
            db_url: None, // in-memory for testing
            persist_state: false,
            prune_minted_premints: false,
            peer_limit: 1000,
            premint_types: "simple".to_string(),
            chain_inclusion_mode: ChainInclusionMode::Check,
            supported_chain_ids: "7777777,".to_string(),
            trusted_peers: None,
        };

        let store = PremintStorage::new(&config).await;
        let premint = PremintTypes::ZoraV2(Default::default());

        store.store(premint.clone()).await.unwrap();
        let retrieved = store.get_for_id(premint.guid()).await.unwrap();
        assert_eq!(premint, retrieved);
    }

    #[tokio::test]
    async fn test_list_all() {
        let config = Config {
            seed: 0,
            port: 7777,
            connect_external: false,
            db_url: None, // in-memory for testing
            persist_state: false,
            prune_minted_premints: false,
            peer_limit: 1000,
            premint_types: "simple".to_string(),
            chain_inclusion_mode: ChainInclusionMode::Check,
            supported_chain_ids: "7777777,".to_string(),
            trusted_peers: None,
        };

        let store = PremintStorage::new(&config).await;

        let premint_v2 = PremintTypes::ZoraV2(Default::default());
        store.store(premint_v2.clone()).await.unwrap();
        let premint_simple = PremintTypes::Simple(Default::default());
        store.store(premint_simple.clone()).await.unwrap();

        let all = store.list_all().await.unwrap();
        assert_eq!(vec![premint_v2, premint_simple], all);
    }
}
