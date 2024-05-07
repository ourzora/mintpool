use std::str::FromStr;

use alloy::primitives::Address;
use async_trait::async_trait;
use eyre::WrapErr;
use serde::{Deserialize, Serialize};
use sqlx::sqlite::SqliteConnectOptions;
use sqlx::Row;
use sqlx::{QueryBuilder, Sqlite, SqlitePool};

use crate::config::Config;
use crate::types::{InclusionClaim, PremintName, PremintTypes};

async fn init_db(config: &Config) -> SqlitePool {
    let expect_msg =
        "Failed to connect to DB. Ensure envar DATABASE_URL is set or ensure PERSIST_STATE=false.";

    if config.persist_state {
        let db_url = config.db_url.clone().expect(expect_msg);
        let opts = SqliteConnectOptions::from_str(&db_url)
            .expect("Failed to parse DB URL")
            .create_if_missing(true);

        SqlitePool::connect_with(opts).await.expect(expect_msg)
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

#[async_trait]
pub trait Writer: Reader {
    async fn store(&self, premint: PremintTypes) -> eyre::Result<()>;
    async fn mark_seen_on_chain(&self, claim: InclusionClaim) -> eyre::Result<()>;
}

#[async_trait]
pub trait Reader: Sync + Send {
    async fn list_all(&self) -> eyre::Result<Vec<PremintTypes>>;
    async fn get_for_id_and_kind(
        &self,
        id: &String,
        kind: PremintName,
    ) -> eyre::Result<PremintTypes>;

    async fn get_for_token_uri(&self, uri: &String) -> eyre::Result<PremintTypes>;
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
}

#[async_trait]
impl Writer for PremintStorage {
    async fn store(&self, premint: PremintTypes) -> eyre::Result<()> {
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

    async fn mark_seen_on_chain(&self, claim: InclusionClaim) -> eyre::Result<()> {
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
}

#[async_trait]
impl Reader for PremintStorage {
    async fn list_all(&self) -> eyre::Result<Vec<PremintTypes>> {
        list_all(&self.db).await
    }

    async fn get_for_id_and_kind(
        &self,
        id: &String,
        kind: PremintName,
    ) -> eyre::Result<PremintTypes> {
        get_for_id_and_kind(&self.db, id, kind).await
    }

    async fn get_for_token_uri(&self, uri: &String) -> eyre::Result<PremintTypes> {
        let row = sqlx::query("SELECT json FROM premints WHERE token_uri = ?")
            .bind(uri)
            .fetch_one(&self.db)
            .await?;
        let json = row.try_get(0)?;
        PremintTypes::from_json(json)
    }
}

pub async fn get_for_id_and_kind(
    db: &SqlitePool,
    id: &String,
    kind: PremintName,
) -> eyre::Result<PremintTypes> {
    let row = sqlx::query(
        r#"
            SELECT json FROM premints WHERE id = ? and kind = ?
        "#,
    )
    .bind(id)
    .bind(kind.0)
    .fetch_one(db)
    .await?;
    let json = row.try_get(0)?;
    PremintTypes::from_json(json)
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

#[derive(Deserialize, Serialize, Debug, Clone, Default)]
pub struct QueryOptions {
    pub chain_id: Option<u64>,
    pub kind: Option<String>,
    pub collection_address: Option<Address>,
    pub creator_address: Option<Address>,
    pub from: Option<chrono::DateTime<chrono::Utc>>,
    pub to: Option<chrono::DateTime<chrono::Utc>>,
}

pub async fn list_all_with_options(
    db: &SqlitePool,
    options: &QueryOptions,
) -> eyre::Result<Vec<PremintTypes>> {
    let mut query = build_query(options);

    let rows = query
        .build()
        .fetch_all(db)
        .await
        .map_err(|e| eyre::eyre!("Failed to list all premints: {}", e))?;

    let premints = rows
        .iter()
        .map(|row| {
            let json: String = row.get("json");
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

pub async fn get_one(db: &SqlitePool, options: &QueryOptions) -> eyre::Result<PremintTypes> {
    let mut query = build_query(options);

    let row = query
        .build()
        .fetch_one(db)
        .await
        .map_err(|e| eyre::eyre!("Failed to list all premints: {}", e))?;

    let json = row.get("json");
    let premint = PremintTypes::from_json(json)?;

    Ok(premint)
}

fn build_query(options: &QueryOptions) -> QueryBuilder<Sqlite> {
    let mut query_build =
        QueryBuilder::<Sqlite>::new("SELECT json FROM premints WHERE seen_on_chain = false");

    if let Some(kind) = options.kind.clone() {
        query_build.push(" AND kind = ");
        query_build.push_bind(kind);
    }
    if let Some(chain_id) = options.chain_id {
        query_build.push(" AND chain_id = ");
        query_build.push_bind(chain_id as i64);
    }
    if let Some(collection_address) = options.collection_address {
        query_build.push(" AND collection_address = ");
        query_build.push_bind(collection_address.to_string());
    }
    if let Some(creator_address) = options.creator_address {
        query_build.push(" AND signer = ");
        query_build.push_bind(creator_address.to_string());
    }
    if let Some(from) = options.from {
        query_build.push(" AND created_at >= ");
        query_build.push_bind(from.to_string());
    }
    if let Some(to) = options.to {
        query_build.push(" AND created_at <= ");
        query_build.push_bind(to.to_string());
    }

    query_build
}

#[cfg(test)]
mod test {
    use std::ops::Sub;

    use alloy::primitives::Address;
    use chrono::{Duration, Utc};
    use sqlx::Row;

    use crate::config::Config;
    use crate::premints::zora_premint::v2::V2;
    use crate::storage;
    use crate::storage::{
        list_all, list_all_with_options, PremintStorage, QueryOptions, Reader, Writer,
    };
    use crate::types::{InclusionClaim, PremintTypes};

    #[tokio::test]
    async fn test_insert_and_get() {
        let config = Config::test_default();

        let store = PremintStorage::new(&config).await;
        let premint = PremintTypes::ZoraV2(Default::default());

        store.store(premint.clone()).await.unwrap();
        let retrieved = store
            .get_for_id_and_kind(&premint.metadata().id, premint.metadata().kind)
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
        if let Ok(_) = store.store(premint.clone()).await {
            panic!("Should not have been able to store premint with same ID")
        }

        // now let's try to update

        let mut premint = V2::default();
        premint.premint.version = 2;
        let premint = PremintTypes::ZoraV2(premint);
        store.store(premint.clone()).await.unwrap();

        let retrieved = store
            .get_for_id_and_kind(&premint.metadata().id.clone(), premint.metadata().kind)
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
    async fn test_list_all_with_options() {
        let config = Config::test_default();

        let store = PremintStorage::new(&config).await;

        let premint_v2 = PremintTypes::ZoraV2(Default::default());
        store.store(premint_v2.clone()).await.unwrap();
        let premint_simple = PremintTypes::Simple(Default::default());
        store.store(premint_simple.clone()).await.unwrap();

        let all = list_all_with_options(
            &store.db,
            &QueryOptions {
                chain_id: Some(0),
                kind: Some("zora_premint_v2".to_string()),
                collection_address: Some(Address::default()),
                creator_address: None,
                from: None,
                to: None,
            },
        )
        .await
        .unwrap();
        assert_eq!(vec![premint_v2.clone()], all);

        let res = list_all(&store.db).await.unwrap();
        assert_eq!(res.len(), 2);

        let all = list_all_with_options(
            &store.db,
            &QueryOptions {
                chain_id: Some(0),
                kind: None,
                collection_address: None,
                creator_address: None,
                from: None,
                to: None,
            },
        )
        .await
        .unwrap();
        assert_eq!(vec![premint_v2.clone(), premint_simple.clone()], all);

        let all = list_all_with_options(
            &store.db,
            &QueryOptions {
                chain_id: None,
                kind: Some("simple".to_string()),
                collection_address: None,
                creator_address: None,
                from: None,
                to: None,
            },
        )
        .await
        .unwrap();
        assert_eq!(vec![premint_simple.clone()], all);

        let to = chrono::Utc::now();

        let from = to.sub(Duration::seconds(10));

        let all = list_all_with_options(
            &store.db,
            &QueryOptions {
                chain_id: None,
                kind: Some("simple".to_string()),
                collection_address: None,
                creator_address: None,
                from: Some(from),
                to: Some(to),
            },
        )
        .await
        .unwrap();
        assert_eq!(vec![premint_simple.clone()], all);
    }

    #[tokio::test]
    async fn test_get_one() {
        let config = Config::test_default();

        let store = PremintStorage::new(&config).await;

        let premint_v2 = PremintTypes::ZoraV2(Default::default());
        store.store(premint_v2.clone()).await.unwrap();
        let premint_simple = PremintTypes::Simple(Default::default());
        store.store(premint_simple.clone()).await.unwrap();

        let all = store.list_all().await.unwrap();
        assert_eq!(all.len(), 2);

        let retrieved = storage::get_one(
            &store.db,
            &QueryOptions {
                chain_id: Some(0),
                kind: Some("simple".to_string()),
                collection_address: None,
                creator_address: None,
                from: None,
                to: Some(Utc::now()),
            },
        )
        .await
        .unwrap();
        assert_eq!(retrieved, premint_simple);
    }

    #[tokio::test]
    async fn test_mark_seen_on_chain() {
        let config = Config {
            prune_minted_premints: true,
            ..Config::test_default()
        };

        let store = PremintStorage::new(&config).await;

        let mut p = V2::default();
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
        let mut p = V2::default();
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
