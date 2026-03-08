use std::collections::BTreeMap;
use std::future::Future;
use std::sync::Arc;

use rehydration_domain::{CaseId, Role, RoleContextPack};

use crate::PortError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NodeProjection {
    pub node_id: String,
    pub node_kind: String,
    pub title: String,
    pub summary: String,
    pub status: String,
    pub labels: Vec<String>,
    pub properties: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NodeRelationProjection {
    pub source_node_id: String,
    pub target_node_id: String,
    pub relation_type: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NodeDetailProjection {
    pub node_id: String,
    pub detail: String,
    pub content_hash: String,
    pub revision: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NodeNeighborhood {
    pub root: NodeProjection,
    pub neighbors: Vec<NodeProjection>,
    pub relations: Vec<NodeRelationProjection>,
}

pub trait ProjectionReader {
    fn load_pack(
        &self,
        case_id: &CaseId,
        role: &Role,
    ) -> impl Future<Output = Result<Option<RoleContextPack>, PortError>> + Send;
}

pub trait GraphNeighborhoodReader {
    fn load_neighborhood(
        &self,
        root_node_id: &str,
    ) -> impl Future<Output = Result<Option<NodeNeighborhood>, PortError>> + Send;
}

pub trait NodeDetailReader {
    fn load_node_detail(
        &self,
        node_id: &str,
    ) -> impl Future<Output = Result<Option<NodeDetailProjection>, PortError>> + Send;
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

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::sync::Arc;

    use rehydration_domain::{CaseHeader, CaseId, Role, RoleContextPack};

    use super::{
        GraphNeighborhoodReader, NodeDetailProjection, NodeDetailReader, NodeNeighborhood,
        NodeProjection, ProjectionReader,
    };
    use crate::PortError;

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

    struct GraphReader;

    impl GraphNeighborhoodReader for GraphReader {
        async fn load_neighborhood(
            &self,
            root_node_id: &str,
        ) -> Result<Option<NodeNeighborhood>, PortError> {
            Ok(Some(NodeNeighborhood {
                root: NodeProjection {
                    node_id: root_node_id.to_string(),
                    node_kind: "capability".to_string(),
                    title: "Projection".to_string(),
                    summary: "seeded neighborhood".to_string(),
                    status: "ACTIVE".to_string(),
                    labels: vec!["projection".to_string()],
                    properties: BTreeMap::new(),
                },
                neighbors: Vec::new(),
                relations: Vec::new(),
            }))
        }
    }

    struct DetailReader;

    impl NodeDetailReader for DetailReader {
        async fn load_node_detail(
            &self,
            node_id: &str,
        ) -> Result<Option<NodeDetailProjection>, PortError> {
            Ok(Some(NodeDetailProjection {
                node_id: node_id.to_string(),
                detail: "expanded detail".to_string(),
                content_hash: "hash-1".to_string(),
                revision: 1,
            }))
        }
    }

    #[tokio::test]
    async fn projection_reader_delegates_through_arc() {
        let reader = Arc::new(Reader);
        let loaded = reader
            .load_pack(
                &CaseId::new("case-123").expect("case id is valid"),
                &Role::new("developer").expect("role is valid"),
            )
            .await
            .expect("load should succeed");

        assert!(loaded.is_some());
    }

    #[tokio::test]
    async fn graph_reader_delegates_through_arc() {
        let reader = Arc::new(GraphReader);
        let loaded = reader
            .load_neighborhood("node-123")
            .await
            .expect("load should succeed");

        assert_eq!(
            loaded.expect("neighborhood should exist").root.node_id,
            "node-123"
        );
    }

    #[tokio::test]
    async fn node_detail_reader_delegates_through_arc() {
        let reader = Arc::new(DetailReader);
        let loaded = reader
            .load_node_detail("node-123")
            .await
            .expect("load should succeed");

        assert_eq!(
            loaded.expect("detail should exist").detail,
            "expanded detail"
        );
    }
}
