use std::error::Error;
use std::fmt;
use std::future::Future;
use std::sync::Arc;

use rehydration_domain::{CaseId, RehydrationBundle, Role, RoleContextPack};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PortError {
    InvalidState(String),
    Unavailable(String),
}

impl fmt::Display for PortError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidState(message) | Self::Unavailable(message) => f.write_str(message),
        }
    }
}

impl Error for PortError {}

pub trait ProjectionReader {
    fn load_pack(
        &self,
        case_id: &CaseId,
        role: &Role,
    ) -> impl Future<Output = Result<Option<RoleContextPack>, PortError>> + Send;
}

pub trait SnapshotStore {
    fn save_bundle(
        &self,
        bundle: &RehydrationBundle,
    ) -> impl Future<Output = Result<(), PortError>> + Send;
}

impl<T> ProjectionReader for Arc<T>
where
    T: ProjectionReader + Send + Sync + ?Sized,
{
    async fn load_pack(
        &self,
        case_id: &CaseId,
        role: &Role,
    ) -> Result<Option<RoleContextPack>, PortError> {
        self.as_ref().load_pack(case_id, role).await
    }
}

impl<T> SnapshotStore for Arc<T>
where
    T: SnapshotStore + Send + Sync + ?Sized,
{
    async fn save_bundle(&self, bundle: &RehydrationBundle) -> Result<(), PortError> {
        self.as_ref().save_bundle(bundle).await
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use rehydration_domain::{CaseHeader, CaseId, RehydrationBundle, Role, RoleContextPack};

    use super::PortError;
    use super::{ProjectionReader, SnapshotStore};

    struct Reader;

    impl ProjectionReader for Reader {
        async fn load_pack(
            &self,
            case_id: &CaseId,
            role: &Role,
        ) -> Result<Option<RoleContextPack>, PortError> {
            Ok(Some(RoleContextPack::new(
                role.clone(),
                CaseHeader::new(
                    case_id.clone(),
                    "Case 123",
                    "A seeded pack",
                    "ACTIVE",
                    std::time::SystemTime::UNIX_EPOCH,
                    "testkit",
                ),
                None,
                Vec::new(),
                Vec::new(),
                Vec::new(),
                Vec::new(),
                Vec::new(),
                "A seeded pack",
                4096,
            )))
        }
    }

    struct Store;

    impl SnapshotStore for Store {
        async fn save_bundle(&self, _bundle: &RehydrationBundle) -> Result<(), PortError> {
            Ok(())
        }
    }

    #[test]
    fn port_error_uses_inner_message() {
        let error = PortError::Unavailable("neo4j unavailable".to_string());
        assert_eq!(error.to_string(), "neo4j unavailable");
    }

    #[tokio::test]
    async fn arc_projection_reader_delegates() {
        let reader = Arc::new(Reader);
        let bundle = reader
            .load_pack(
                &CaseId::new("case-123").expect("case id is valid"),
                &Role::new("developer").expect("role is valid"),
            )
            .await
            .expect("load should succeed");

        assert!(bundle.is_some());
    }

    #[tokio::test]
    async fn arc_snapshot_store_delegates() {
        let store = Arc::new(Store);
        let bundle = RehydrationBundle::empty(
            CaseId::new("case-123").expect("case id is valid"),
            Role::new("developer").expect("role is valid"),
            "0.1.0",
        );

        store
            .save_bundle(&bundle)
            .await
            .expect("save via arc should succeed");
    }
}
