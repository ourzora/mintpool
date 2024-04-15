use crate::config::Config;
use crate::types::{InclusionClaim, Premint, PremintName, PremintTypes};
use eyre::WrapErr;
use sqlx::{Row, SqlitePool};
use std::fs;

async fn init_db(config: &Config) -> SqlitePool {
    let expect_msg =
        "Failed to connect to DB. Ensure envar DATABASE_URL is set or ensure PERSIST_STATE=false.";

    if config.persist_state {
        let db_url = config.db_url.clone().expect(expect_msg);

        if fs::metadata(db_url.clone()).is_err() {
            fs::File::create(db_url.clone()).expect("Failed to create DB file");
        }
        SqlitePool::connect(&db_url).await.expect(expect_msg)
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
        let chain_id = metadata.chain_id as i64;
        let version = metadata.version as i64;
        let token_uri = metadata.uri;

        let result = sqlx::query!(
            r#"
            INSERT INTO premints (id, kind, version, signer, chain_id, collection_address, token_id, token_uri, json)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            ON CONFLICT (kind, id) DO UPDATE SET version = $3, json = $9
            WHERE excluded.version > version;
        "#,
            metadata.id,
            metadata.kind.0,
            version,
            signer,
            chain_id,
            collection_address,
            token_id,
            token_uri,
            json,
        )
        .execute(&self.db)
        .await
        .map_err(|e| eyre::eyre!("Failed to store premint: {}", e))?;

        // no rows affected means the version was not higher that what's already stored
        if result.rows_affected() == 0 {
            return Err(eyre::eyre!(
                "Cannot store premint with lower version than existing"
            ));
        }

        Ok(())
    }

    pub async fn mark_seen_on_chain(&self, claim: InclusionClaim) -> eyre::Result<()> {
        let chain_id = claim.chain_id as i64;
        if self.prune_minted_premints {
            let r = sqlx::query!(
                r#"
                DELETE FROM premints WHERE id = ? AND chain_id = ? AND kind = ?
            "#,
                claim.premint_id,
                chain_id,
                claim.kind
            )
            .execute(&self.db)
            .await
            .map_err(|e| eyre::eyre!("Failed to delete premint: {}", e))?;
            tracing::debug!("Rows affected pruning: {}", r.rows_affected())
        } else {
            let r = sqlx::query!(
                r#"
                UPDATE premints SET seen_on_chain = true WHERE id = ? AND chain_id = ? AND kind = ?
            "#,
                claim.premint_id,
                chain_id,
                claim.kind
            )
            .execute(&self.db)
            .await
            .map_err(|e| eyre::eyre!("Failed to update premint: {}", e))?;
            tracing::debug!("Rows affected marking: {}", r.rows_affected())
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

    pub async fn get_for_token_uri(&self, uri: String) -> eyre::Result<PremintTypes> {
        let row = sqlx::query("SELECT json FROM premints WHERE token_uri = ?")
            .bind(uri)
            .fetch_one(&self.db)
            .await?;
        let json = row.try_get(0)?;
        PremintTypes::from_json(json)
    }
}

pub async fn list_all(db: &SqlitePool) -> eyre::Result<Vec<PremintTypes>> {
    let rows = sqlx::query(
        r#"
            SELECT json FROM premints WHERE seen_on_chain = false
        "#,
    )
    .fetch_all(db)
    .await
    .map_err(|e| eyre::eyre!("Failed to list all premints: {}", e))?;
    let premints = rows
        .iter()
        .map(|row| {
            let json: String = row.get(0);
            PremintTypes::from_json(json)
        })
        .filter_map(|i| match i {
            Ok(p) => Some(p),
            Err(e) => {
                tracing::warn!("Failed to deserialize premint in db: {}", e);
                None
            }
        })
        .collect();

    Ok(premints)
}

#[cfg(test)]
mod test {
    use crate::config::{ChainInclusionMode, Config};
    use crate::premints::zora_premint_v2::types::ZoraPremintV2;
    use crate::storage::PremintStorage;
    use crate::types::{InclusionClaim, Premint, PremintTypes};
    use alloy_primitives::U256;
    use sqlx::Row;

    #[tokio::test]
    async fn test_insert_and_get() {
        let config = Config::test_default();

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
    async fn test_update() {
        let config = Config::test_default();

        let store = PremintStorage::new(&config).await;
        let premint = PremintTypes::ZoraV2(Default::default());

        // first should work
        match store.store(premint.clone()).await {
            Ok(_) => {}
            Err(e) => panic!("Failed to store premint: {}", e),
        }

        // second should fail
        match store.store(premint.clone()).await {
            Ok(_) => panic!("Should not have been able to store premint with same ID"),
            Err(_) => {}
        }

        // now let's try to update

        let mut premint = ZoraPremintV2::default();
        premint.premint.version = 2;
        let premint = PremintTypes::ZoraV2(premint);
        store.store(premint.clone()).await.unwrap();

        let retrieved = store
            .get_for_id_and_kind(premint.metadata().id.clone(), premint.metadata().kind)
            .await
            .unwrap();
        assert_eq!(premint, retrieved);
    }

    #[tokio::test]
    async fn test_list_all() {
        let config = Config::test_default();

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
            prune_minted_premints: true,
            ..Config::test_default()
        };

        let store = PremintStorage::new(&config).await;

        let mut p = ZoraPremintV2::default();
        p.premint.uid = 1;
        p.chain_id = 7777777;
        let premint_v2 = PremintTypes::ZoraV2(p);
        store.store(premint_v2.clone()).await.unwrap();
        let premint_simple = PremintTypes::Simple(Default::default());
        store.store(premint_simple.clone()).await.unwrap();

        let all = store.list_all().await.unwrap();
        assert_eq!(all.len(), 2);
        store
            .mark_seen_on_chain(InclusionClaim {
                premint_id: premint_v2.metadata().id.clone(),
                chain_id: 7777777,
                tx_hash: Default::default(),
                log_index: 0,
                kind: "zora_premint_v2".to_string(),
            })
            .await
            .unwrap();

        let all = store.list_all().await.unwrap();
        assert_eq!(all.len(), 1);
    }

    #[tokio::test]
    async fn test_prune_false_keeps_seen_premints() {
        let mut config = Config::test_default();
        config.prune_minted_premints = false;

        let store = PremintStorage::new(&config).await;

        // Make sure IDs are different
        let mut p = ZoraPremintV2::default();
        p.premint.uid = 1;
        p.chain_id = 7777777;
        let premint_v2 = PremintTypes::ZoraV2(p);

        store.store(premint_v2.clone()).await.unwrap();
        let premint_simple = PremintTypes::Simple(Default::default());
        store.store(premint_simple.clone()).await.unwrap();

        let all = store.list_all().await.unwrap();
        assert_eq!(all.len(), 2);
        store
            .mark_seen_on_chain(InclusionClaim {
                premint_id: premint_v2.metadata().id.clone(),
                chain_id: 7777777,
                tx_hash: Default::default(),
                log_index: 0,
                kind: "zora_premint_v2".to_string(),
            })
            .await
            .unwrap();

        let all = sqlx::query("SELECT count(*) as c FROM premints")
            .fetch_one(&store.db())
            .await
            .unwrap();
        let count: i64 = all.try_get("c").unwrap();
        assert_eq!(count, 2);

        let res = sqlx::query("SELECT count(*) as c FROM premints WHERE seen_on_chain = true")
            .fetch_one(&store.db())
            .await
            .unwrap();

        let count: i64 = res.try_get("c").unwrap();
        assert_eq!(count, 1);

        let res = sqlx::query("SELECT count(*) as c FROM premints WHERE seen_on_chain = false")
            .fetch_one(&store.db())
            .await
            .unwrap();

        let count: i64 = res.try_get("c").unwrap();
        assert_eq!(count, 1);
    }
}
