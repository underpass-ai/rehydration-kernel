use rehydration_domain::RehydrationBundle;
use rehydration_ports::{PortError, SnapshotStore};

use crate::adapter::bundle_serialization::serialize_bundle;
use crate::adapter::endpoint::ValkeyEndpoint;
use crate::adapter::io::execute_set_command;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValkeySnapshotStore {
    pub(crate) endpoint: ValkeyEndpoint,
}

impl ValkeySnapshotStore {
    pub fn new(snapshot_uri: impl Into<String>) -> Result<Self, PortError> {
        let endpoint = ValkeyEndpoint::parse(snapshot_uri.into())?;
        Ok(Self { endpoint })
    }

    pub(crate) fn snapshot_key(&self, bundle: &RehydrationBundle) -> String {
        format!(
            "{}:{}:{}",
            self.endpoint.key_prefix,
            bundle.root_node_id().as_str(),
            bundle.role().as_str()
        )
    }

    pub(crate) fn snapshot_payload(&self, bundle: &RehydrationBundle) -> Result<String, PortError> {
        serialize_bundle(bundle)
    }

    async fn execute_set_command(&self, key: &str, payload: &str) -> Result<(), PortError> {
        execute_set_command(&self.endpoint, key, payload).await
    }
}

impl SnapshotStore for ValkeySnapshotStore {
    async fn save_bundle(&self, bundle: &RehydrationBundle) -> Result<(), PortError> {
        let key = self.snapshot_key(bundle);
        let payload = self.snapshot_payload(bundle)?;
        self.execute_set_command(&key, &payload).await
    }
}
