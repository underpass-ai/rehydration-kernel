use std::sync::Arc;

#[derive(Debug)]
pub struct AdminQueryApplicationService<G, D> {
    pub(crate) graph_reader: Arc<G>,
    pub(crate) detail_reader: Arc<D>,
    pub(crate) generator_version: &'static str,
}

impl<G, D> AdminQueryApplicationService<G, D> {
    pub fn new(
        graph_reader: Arc<G>,
        detail_reader: Arc<D>,
        generator_version: &'static str,
    ) -> Self {
        Self {
            graph_reader,
            detail_reader,
            generator_version,
        }
    }
}
