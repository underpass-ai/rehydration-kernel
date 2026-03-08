use std::future::Future;
use std::sync::Arc;

use crate::{NodeNeighborhood, PortError};

pub trait GraphNeighborhoodReader {
    fn load_neighborhood(
        &self,
        root_node_id: &str,
    ) -> impl Future<Output = Result<Option<NodeNeighborhood>, PortError>> + Send;
}

impl<T> GraphNeighborhoodReader for Arc<T>
where
    T: GraphNeighborhoodReader + Send + Sync + ?Sized,
{
    async fn load_neighborhood(
        &self,
        root_node_id: &str,
    ) -> Result<Option<NodeNeighborhood>, PortError> {
        self.as_ref().load_neighborhood(root_node_id).await
    }
}

impl<T> GraphNeighborhoodReader for &T
where
    T: GraphNeighborhoodReader + Send + Sync + ?Sized,
{
    async fn load_neighborhood(
        &self,
        root_node_id: &str,
    ) -> Result<Option<NodeNeighborhood>, PortError> {
        (*self).load_neighborhood(root_node_id).await
    }
}
