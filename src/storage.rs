use crate::config::Config;
use crate::types::{InclusionClaim, Premint, PremintName, PremintTypes};
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

impl Clone for PremintStorage {
    fn clone(&self) -> Self {
        Self {
            db: self.db.clone(),
            // we want at most one instance to prune premints,
            // so we'll always set it to false when cloning
            prune_minted_premints: false,
        }
    }
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
        let signer = metadata.signer.to_checksum(None);
        let collection_address = metadata.collection_address.to_checksum(None);
        let token_id = metadata.token_id.to_string();
        let chain_id = metadata.chain_id.to::<i64>();
        sqlx::query!(
            r#"
            INSERT INTO premints (id, kind, signer, chain_id, collection_address, token_id, json)
            VALUES (?, ?, ?, ?, ?, ?, ?)
        "#,
            metadata.id,
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

    pub async fn mark_seen_on_chain(&self, claim: InclusionClaim) -> eyre::Result<()> {
        if self.prune_minted_premints {
            sqlx::query!(
                r#"
                DELETE FROM premints WHERE id = ?
            "#,
                claim.premint_id
            )
            .execute(&self.db)
            .await
            .map_err(|e| eyre::eyre!("Failed to delete premint: {}", e))?;
        } else {
            sqlx::query!(
                r#"
                UPDATE premints SET seen_on_chain = true WHERE id = ?
            "#,
                claim.premint_id
            )
            .execute(&self.db)
            .await
            .map_err(|e| eyre::eyre!("Failed to update premint: {}", e))?;
        }

        Ok(())
    }

    pub async fn list_all(&self) -> eyre::Result<Vec<PremintTypes>> {
        list_all(&self.db).await
    }

    pub async fn get_for_id_and_kind(
        &self,
        id: String,
        kind: PremintName,
    ) -> eyre::Result<PremintTypes> {
        let row = sqlx::query(
            r#"
            SELECT json FROM premints WHERE id = ? and kind = ?
        "#,
        )
        .bind(id)
        .bind(kind.0)
        .fetch_one(&self.db)
        .await?;
        let json = row.try_get(0)?;
        PremintTypes::from_json(json)
    }
}

pub async fn list_all(db: &SqlitePool) -> eyre::Result<Vec<PremintTypes>> {
    let rows = sqlx::query(
        r#"
            SELECT json FROM premints
        "#,
    )
    .fetch_all(db)
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

#[cfg(test)]
mod test {
    use crate::config::{ChainInclusionMode, Config};
    use crate::storage::PremintStorage;
    use crate::types::{InclusionClaim, Premint, PremintTypes};

    #[tokio::test]
    async fn test_insert_and_get() {
        let config = Config {
            seed: 0,
            peer_port: 7777,
            connect_external: false,
            db_url: None, // in-memory for testing
            persist_state: false,
            prune_minted_premints: false,
            peer_limit: 1000,
            premint_types: "simple".to_string(),
            chain_inclusion_mode: ChainInclusionMode::Check,
            supported_chain_ids: "7777777,".to_string(),
            trusted_peers: None,
            api_port: 0,
            node_id: None,
            interactive: false,
            external_address: None,
        };

        let store = PremintStorage::new(&config).await;
        let premint = PremintTypes::ZoraV2(Default::default());

        store.store(premint.clone()).await.unwrap();
        let retrieved = store
            .get_for_id_and_kind(premint.metadata().id, premint.metadata().kind)
            .await
            .unwrap();
        assert_eq!(premint, retrieved);
    }

    #[tokio::test]
    async fn test_list_all() {
        let config = Config {
            seed: 0,
            peer_port: 7778,
            connect_external: false,
            db_url: None, // in-memory for testing
            persist_state: false,
            prune_minted_premints: false,
            api_port: 7777,
            peer_limit: 1000,
            premint_types: "simple".to_string(),
            chain_inclusion_mode: ChainInclusionMode::Check,
            supported_chain_ids: "7777777,".to_string(),
            trusted_peers: None,
            node_id: None,
            interactive: false,
            external_address: None,
        };

        let store = PremintStorage::new(&config).await;

        let premint_v2 = PremintTypes::ZoraV2(Default::default());
        store.store(premint_v2.clone()).await.unwrap();
        let premint_simple = PremintTypes::Simple(Default::default());
        store.store(premint_simple.clone()).await.unwrap();

        let all = store.list_all().await.unwrap();
        assert_eq!(vec![premint_v2, premint_simple], all);
    }

    #[tokio::test]
    async fn test_mark_seen_on_chain() {
        let config = Config {
            seed: 0,
            connect_external: false,
            db_url: None, // in-memory for testing
            persist_state: false,
            prune_minted_premints: true,
            api_port: 0,
            peer_limit: 1000,
            premint_types: "simple".to_string(),
            chain_inclusion_mode: ChainInclusionMode::Check,
            supported_chain_ids: "7777777,".to_string(),
            trusted_peers: None,
            node_id: None,
            external_address: None,
            peer_port: 0,
            interactive: false,
        };

        let store = PremintStorage::new(&config).await;

        let premint_v2 = PremintTypes::V2(Default::default());
        store.store(premint_v2.clone()).await.unwrap();
        let premint_simple = PremintTypes::Simple(Default::default());
        store.store(premint_simple.clone()).await.unwrap();

        let all = store.list_all().await.unwrap();
        assert_eq!(all.len(), 2);
        store
            .mark_seen_on_chain(InclusionClaim {
                premint_id: premint_v2.metadata().id.clone(),
                chain_id: 0,
                tx_hash: Default::default(),
                log_index: 0,
                kind: "".to_string(),
            })
            .await
            .unwrap();

        let all = store.list_all().await.unwrap();
        assert_eq!(all.len(), 1);
    }

    #[tokio::test]
    async fn test_prune_false_keeps_seen_premints() {
        let config = Config {
            seed: 0,
            connect_external: false,
            db_url: None, // in-memory for testing
            persist_state: false,
            prune_minted_premints: false,
            api_port: 0,
            peer_limit: 1000,
            premint_types: "simple".to_string(),
            chain_inclusion_mode: ChainInclusionMode::Check,
            supported_chain_ids: "7777777,".to_string(),
            trusted_peers: None,
            node_id: None,
            external_address: None,
            peer_port: 0,
            interactive: false,
        };

        let store = PremintStorage::new(&config).await;

        let premint_v2 = PremintTypes::V2(Default::default());
        store.store(premint_v2.clone()).await.unwrap();
        let premint_simple = PremintTypes::Simple(Default::default());
        store.store(premint_simple.clone()).await.unwrap();

        let all = store.list_all().await.unwrap();
        assert_eq!(all.len(), 2);
        store
            .mark_seen_on_chain(InclusionClaim {
                premint_id: premint_v2.metadata().id.clone(),
                chain_id: 0,
                tx_hash: Default::default(),
                log_index: 0,
                kind: "".to_string(),
            })
            .await
            .unwrap();

        let all = store.list_all().await.unwrap();
        assert_eq!(all.len(), 2);

        let res = sqlx::query("SELECT * FROM premints WHERE seen_on_chain = true")
            .execute(&store.db())
            .await
            .unwrap();

        assert_eq!(res.rows_affected(), 1);
    }
}
