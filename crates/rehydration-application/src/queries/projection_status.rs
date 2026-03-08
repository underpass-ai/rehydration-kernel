use crate::queries::AdminQueryApplicationService;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GetProjectionStatusQuery {
    pub consumer_names: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectionStatusView {
    pub consumer_name: String,
    pub stream_name: String,
    pub projection_watermark: String,
    pub processed_events: u64,
    pub pending_events: u64,
    pub last_event_at: std::time::SystemTime,
    pub updated_at: std::time::SystemTime,
    pub healthy: bool,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GetProjectionStatusResult {
    pub projections: Vec<ProjectionStatusView>,
    pub observed_at: std::time::SystemTime,
}

#[derive(Debug, Default, Clone, Copy)]
pub struct GetProjectionStatusUseCase;

impl GetProjectionStatusUseCase {
    pub fn execute(&self, query: GetProjectionStatusQuery) -> GetProjectionStatusResult {
        let observed_at = std::time::SystemTime::now();
        let consumer_names = if query.consumer_names.is_empty() {
            vec!["context-projection".to_string()]
        } else {
            query.consumer_names
        };

        GetProjectionStatusResult {
            projections: consumer_names
                .into_iter()
                .map(|consumer_name| ProjectionStatusView {
                    stream_name: format!("{consumer_name}.events"),
                    consumer_name,
                    projection_watermark: "rev-0".to_string(),
                    processed_events: 0,
                    pending_events: 0,
                    last_event_at: observed_at,
                    updated_at: observed_at,
                    healthy: true,
                    warnings: vec!["projection status is placeholder-backed".to_string()],
                })
                .collect(),
            observed_at,
        }
    }
}

impl<G, D> AdminQueryApplicationService<G, D> {
    pub fn get_projection_status(
        &self,
        query: GetProjectionStatusQuery,
    ) -> GetProjectionStatusResult {
        GetProjectionStatusUseCase.execute(query)
    }
}
