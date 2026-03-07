use rehydration_domain::{BundleMetadata, CaseId, RehydrationBundle, Role};
use rehydration_ports::{PortError, ProjectionReader};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Neo4jProjectionReader {
    graph_uri: String,
}

impl Neo4jProjectionReader {
    pub fn new(graph_uri: String) -> Self {
        Self { graph_uri }
    }
}

impl ProjectionReader for Neo4jProjectionReader {
    async fn load_bundle(
        &self,
        case_id: &CaseId,
        role: &Role,
    ) -> Result<Option<RehydrationBundle>, PortError> {
        let section = format!(
            "projection placeholder from {} for case {} and role {}",
            self.graph_uri,
            case_id.as_str(),
            role.as_str()
        );

        Ok(Some(RehydrationBundle::new(
            case_id.clone(),
            role.clone(),
            vec![section],
            BundleMetadata::initial("neo4j-placeholder"),
        )))
    }
}

#[cfg(test)]
mod tests {
    use rehydration_domain::{CaseId, Role};
    use rehydration_ports::ProjectionReader;

    use super::Neo4jProjectionReader;

    #[tokio::test]
    async fn adapter_returns_a_placeholder_bundle() {
        let reader = Neo4jProjectionReader::new("neo4j://localhost:7687".to_string());
        let bundle = reader
            .load_bundle(
                &CaseId::new("case-123").expect("case id is valid"),
                &Role::new("developer").expect("role is valid"),
            )
            .await
            .expect("bundle load should succeed")
            .expect("placeholder bundle should exist");

        assert_eq!(bundle.metadata().generator_version, "neo4j-placeholder");
    }
}
