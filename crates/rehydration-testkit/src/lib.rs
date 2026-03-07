use std::collections::HashMap;

use rehydration_domain::{CaseId, RehydrationBundle, Role};
use rehydration_ports::{PortError, ProjectionReader, SnapshotStore};

#[derive(Debug, Default, Clone)]
pub struct InMemoryProjectionReader {
    bundles: HashMap<(CaseId, Role), RehydrationBundle>,
}

impl InMemoryProjectionReader {
    pub fn with_bundle(bundle: RehydrationBundle) -> Self {
        let key = (bundle.case_id().clone(), bundle.role().clone());
        let mut bundles = HashMap::new();
        bundles.insert(key, bundle);
        Self { bundles }
    }
}

impl ProjectionReader for InMemoryProjectionReader {
    async fn load_bundle(
        &self,
        case_id: &CaseId,
        role: &Role,
    ) -> Result<Option<RehydrationBundle>, PortError> {
        Ok(self.bundles.get(&(case_id.clone(), role.clone())).cloned())
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub struct NoopSnapshotStore;

impl SnapshotStore for NoopSnapshotStore {
    async fn save_bundle(&self, _bundle: &RehydrationBundle) -> Result<(), PortError> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use rehydration_domain::{CaseId, RehydrationBundle, Role};
    use rehydration_ports::ProjectionReader;

    use super::InMemoryProjectionReader;

    #[tokio::test]
    async fn in_memory_reader_returns_seeded_bundle() {
        let bundle = RehydrationBundle::empty(
            CaseId::new("case-123").expect("case id is valid"),
            Role::new("developer").expect("role is valid"),
            "0.1.0",
        );
        let reader = InMemoryProjectionReader::with_bundle(bundle);

        let loaded = reader
            .load_bundle(
                &CaseId::new("case-123").expect("case id is valid"),
                &Role::new("developer").expect("role is valid"),
            )
            .await
            .expect("load should succeed");

        assert!(loaded.is_some());
    }
}
