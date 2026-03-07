use std::collections::HashMap;

use rehydration_domain::{CaseId, RehydrationBundle, Role, RoleContextPack};
use rehydration_ports::{PortError, ProjectionReader, SnapshotStore};

#[derive(Debug, Default, Clone)]
pub struct InMemoryProjectionReader {
    packs: HashMap<(CaseId, Role), RoleContextPack>,
}

impl InMemoryProjectionReader {
    pub fn with_pack(pack: RoleContextPack) -> Self {
        let key = (pack.case_header().case_id().clone(), pack.role().clone());
        let mut packs = HashMap::new();
        packs.insert(key, pack);
        Self { packs }
    }

    pub fn with_bundle(bundle: RehydrationBundle) -> Self {
        Self::with_pack(bundle.pack().clone())
    }
}

impl ProjectionReader for InMemoryProjectionReader {
    async fn load_pack(
        &self,
        case_id: &CaseId,
        role: &Role,
    ) -> Result<Option<RoleContextPack>, PortError> {
        Ok(self.packs.get(&(case_id.clone(), role.clone())).cloned())
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
    use rehydration_ports::{ProjectionReader, SnapshotStore};

    use super::{InMemoryProjectionReader, NoopSnapshotStore};

    #[tokio::test]
    async fn in_memory_reader_returns_seeded_bundle() {
        let bundle = RehydrationBundle::empty(
            CaseId::new("case-123").expect("case id is valid"),
            Role::new("developer").expect("role is valid"),
            "0.1.0",
        );
        let reader = InMemoryProjectionReader::with_bundle(bundle);

        let loaded = reader
            .load_pack(
                &CaseId::new("case-123").expect("case id is valid"),
                &Role::new("developer").expect("role is valid"),
            )
            .await
            .expect("load should succeed");

        assert!(loaded.is_some());
    }

    #[tokio::test]
    async fn noop_snapshot_store_accepts_bundle() {
        let bundle = RehydrationBundle::empty(
            CaseId::new("case-123").expect("case id is valid"),
            Role::new("developer").expect("role is valid"),
            "0.1.0",
        );

        NoopSnapshotStore
            .save_bundle(&bundle)
            .await
            .expect("noop snapshot store should accept bundles");
    }
}
