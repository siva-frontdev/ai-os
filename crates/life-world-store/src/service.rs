//! [`Service`] adapter for the World Store (RFC-0008 §Integration).

use std::sync::Arc;

use ai_os_core::error::CoreError;
use ai_os_core::lifecycle::Service;
use async_trait::async_trait;

use crate::error::WorldStoreError;
use crate::store::SqliteWorldModelStore;
use crate::types::WorldStoreConfig;

/// Long-lived [`SqliteWorldModelStore`] wrapped in the core [`Service`]
/// lifecycle. `start` opens the database and runs the optional
/// `world_model.json` migration; `stop` checkpoints and closes the connection.
#[derive(Debug)]
pub struct WorldStoreService {
    store: Arc<SqliteWorldModelStore>,
    config: WorldStoreConfig,
}

impl WorldStoreService {
    /// Wrap a store (optionally with an event bus) into a service.
    pub fn new(store: Arc<SqliteWorldModelStore>) -> Self {
        let config = WorldStoreConfig::default();
        Self { store, config }
    }

    /// Wrap a store with explicit configuration.
    pub fn with_config(store: Arc<SqliteWorldModelStore>, config: WorldStoreConfig) -> Self {
        Self { store, config }
    }

    /// The underlying store.
    pub fn store(&self) -> &Arc<SqliteWorldModelStore> {
        &self.store
    }

    fn map_err(e: WorldStoreError) -> CoreError {
        CoreError::General(format!("world store: {e}"))
    }
}

#[async_trait]
impl Service for WorldStoreService {
    fn name(&self) -> &str {
        "life.world_store"
    }

    async fn start(&self) -> Result<(), CoreError> {
        self.store.init().await.map_err(Self::map_err)?;
        if let Some(source) = &self.config.json_migration_source {
            let report = self
                .store
                .migrate_from_json(source)
                .await
                .map_err(Self::map_err)?;
            tracing::info!(
                skipped = report.skipped,
                entities = report.entities_imported,
                relationships = report.relationships_imported,
                "world store world_model.json migration"
            );
        }
        Ok(())
    }

    async fn stop(&self) -> Result<(), CoreError> {
        self.store.close().await.map_err(Self::map_err)
    }
}
