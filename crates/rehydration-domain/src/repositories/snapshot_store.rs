use std::future::Future;
use std::sync::Arc;

use crate::{PortError, RehydrationBundle};

pub trait SnapshotStore {
    fn save_bundle(
        &self,
        bundle: &RehydrationBundle,
    ) -> impl Future<Output = Result<(), PortError>> + Send;
}

impl<T> SnapshotStore for Arc<T>
where
    T: SnapshotStore + Send + Sync + ?Sized,
{
    async fn save_bundle(&self, bundle: &RehydrationBundle) -> Result<(), PortError> {
        self.as_ref().save_bundle(bundle).await
    }
}
