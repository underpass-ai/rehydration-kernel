use std::future::Future;
use std::sync::Arc;

use crate::{NodeDetailProjection, PortError};

pub trait NodeDetailReader {
    fn load_node_detail(
        &self,
        node_id: &str,
    ) -> impl Future<Output = Result<Option<NodeDetailProjection>, PortError>> + Send;
}

impl<T> NodeDetailReader for Arc<T>
where
    T: NodeDetailReader + Send + Sync + ?Sized,
{
    async fn load_node_detail(
        &self,
        node_id: &str,
    ) -> Result<Option<NodeDetailProjection>, PortError> {
        self.as_ref().load_node_detail(node_id).await
    }
}

impl<T> NodeDetailReader for &T
where
    T: NodeDetailReader + Send + Sync + ?Sized,
{
    async fn load_node_detail(
        &self,
        node_id: &str,
    ) -> Result<Option<NodeDetailProjection>, PortError> {
        (*self).load_node_detail(node_id).await
    }
}
