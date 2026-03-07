use rehydration_domain::RehydrationBundle;
use rehydration_ports::{PortError, SnapshotStore};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValkeySnapshotStore {
    snapshot_uri: String,
}

impl ValkeySnapshotStore {
    pub fn new(snapshot_uri: String) -> Self {
        Self { snapshot_uri }
    }
}

impl SnapshotStore for ValkeySnapshotStore {
    fn save_bundle(&self, bundle: &RehydrationBundle) -> Result<(), PortError> {
        if self.snapshot_uri.trim().is_empty() {
            return Err(PortError::InvalidState(
                "snapshot uri cannot be empty".to_string(),
            ));
        }

        let _revision = bundle.metadata().revision;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use rehydration_domain::{CaseId, RehydrationBundle, Role};
    use rehydration_ports::SnapshotStore;

    use super::ValkeySnapshotStore;

    #[test]
    fn snapshot_store_accepts_non_empty_uri() {
        let store = ValkeySnapshotStore::new("redis://localhost:6379".to_string());
        let bundle = RehydrationBundle::empty(
            CaseId::new("case-123").expect("case id is valid"),
            Role::new("reviewer").expect("role is valid"),
            "0.1.0",
        );

        store
            .save_bundle(&bundle)
            .expect("non-empty uri should be accepted");
    }
}
