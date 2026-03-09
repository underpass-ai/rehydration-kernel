use rehydration_domain::{
    GraphNeighborhoodReader, NodeDetailProjection, NodeDetailReader, NodeNeighborhood, PortError,
    RehydrationBundle, SnapshotSaveOptions, SnapshotStore,
};

pub(crate) struct EmptyGraphNeighborhoodReader;

impl GraphNeighborhoodReader for EmptyGraphNeighborhoodReader {
    async fn load_neighborhood(
        &self,
        _root_node_id: &str,
    ) -> Result<Option<NodeNeighborhood>, PortError> {
        Ok(None)
    }
}

pub(crate) struct EmptyNodeDetailReader;

impl NodeDetailReader for EmptyNodeDetailReader {
    async fn load_node_detail(
        &self,
        _node_id: &str,
    ) -> Result<Option<NodeDetailProjection>, PortError> {
        Ok(None)
    }
}

pub(crate) struct NoopSnapshotStore;

impl SnapshotStore for NoopSnapshotStore {
    async fn save_bundle_with_options(
        &self,
        _bundle: &RehydrationBundle,
        _options: SnapshotSaveOptions,
    ) -> Result<(), PortError> {
        Ok(())
    }
}
