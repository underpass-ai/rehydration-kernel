use std::collections::BTreeSet;

use rehydration_domain::{
    BundleMetadata, BundleNode, BundleNodeDetail, BundleRelationship, CaseId,
    GraphNeighborhoodReader, NodeDetailReader, NodeNeighborhood, RehydrationBundle, Role,
};

use crate::ApplicationError;
use crate::queries::ordered_neighborhood::ordered_neighborhood;
use crate::queries::{DEFAULT_NATIVE_GRAPH_TRAVERSAL_DEPTH, clamp_native_graph_traversal_depth};

#[derive(Debug, Clone)]
pub struct NodeCentricProjectionReader<G, D> {
    graph_reader: G,
    detail_reader: D,
}

impl<G, D> NodeCentricProjectionReader<G, D> {
    pub fn new(graph_reader: G, detail_reader: D) -> Self {
        Self {
            graph_reader,
            detail_reader,
        }
    }
}

impl<G, D> NodeCentricProjectionReader<G, D>
where
    G: GraphNeighborhoodReader + Send + Sync,
    D: NodeDetailReader + Send + Sync,
{
    pub async fn load_bundle(
        &self,
        root_node_id: &str,
        role: &str,
        generator_version: &str,
    ) -> Result<Option<RehydrationBundle>, ApplicationError> {
        self.load_bundle_with_depth(
            root_node_id,
            role,
            generator_version,
            DEFAULT_NATIVE_GRAPH_TRAVERSAL_DEPTH,
        )
        .await
    }

    pub async fn load_bundle_with_depth(
        &self,
        root_node_id: &str,
        role: &str,
        generator_version: &str,
        depth: u32,
    ) -> Result<Option<RehydrationBundle>, ApplicationError> {
        let Some(neighborhood) = self
            .graph_reader
            .load_neighborhood(root_node_id, clamp_native_graph_traversal_depth(depth))
            .await?
        else {
            return Ok(None);
        };

        let neighborhood = ordered_neighborhood(neighborhood);
        let node_details = load_node_details(&self.detail_reader, &neighborhood).await?;

        Ok(Some(build_bundle(
            root_node_id,
            role,
            generator_version,
            neighborhood,
            node_details,
        )?))
    }

    pub async fn load_context_path_bundle_with_depth(
        &self,
        root_node_id: &str,
        target_node_id: &str,
        role: &str,
        generator_version: &str,
        subtree_depth: u32,
    ) -> Result<Option<RehydrationBundle>, ApplicationError> {
        let Some(path_neighborhood) = self
            .graph_reader
            .load_context_path(
                root_node_id,
                target_node_id,
                clamp_native_graph_traversal_depth(subtree_depth),
            )
            .await?
        else {
            return Ok(None);
        };

        let path_node_ids = path_neighborhood.path_node_ids.clone();
        let neighborhood = ordered_neighborhood(NodeNeighborhood {
            root: path_neighborhood.root,
            neighbors: path_neighborhood.neighbors,
            relations: path_neighborhood.relations,
        });
        let node_details = load_node_details_for_ids(&self.detail_reader, path_node_ids).await?;

        Ok(Some(build_bundle(
            root_node_id,
            role,
            generator_version,
            neighborhood,
            node_details,
        )?))
    }
}

async fn load_node_details<D>(
    detail_reader: &D,
    neighborhood: &NodeNeighborhood,
) -> Result<Vec<BundleNodeDetail>, rehydration_domain::PortError>
where
    D: NodeDetailReader + Send + Sync,
{
    load_node_details_for_ids(
        detail_reader,
        std::iter::once(neighborhood.root.node_id.clone())
            .chain(
                neighborhood
                    .neighbors
                    .iter()
                    .map(|node| node.node_id.clone()),
            )
            .collect::<Vec<_>>(),
    )
    .await
}

async fn load_node_details_for_ids<D, I>(
    detail_reader: &D,
    node_ids: I,
) -> Result<Vec<BundleNodeDetail>, rehydration_domain::PortError>
where
    D: NodeDetailReader + Send + Sync,
    I: IntoIterator<Item = String>,
{
    let mut details = Vec::new();
    let mut seen = BTreeSet::new();

    for node_id in node_ids {
        if !seen.insert(node_id.clone()) {
            continue;
        }

        if let Some(detail) = detail_reader.load_node_detail(&node_id).await? {
            details.push(BundleNodeDetail::from_projection(&detail));
        }
    }

    Ok(details)
}

fn build_bundle(
    root_node_id: &str,
    role: &str,
    generator_version: &str,
    neighborhood: NodeNeighborhood,
    node_details: Vec<BundleNodeDetail>,
) -> Result<RehydrationBundle, ApplicationError> {
    let root_node_id = CaseId::new(root_node_id)?;
    let role = Role::new(role)?;

    Ok(RehydrationBundle::new(
        root_node_id,
        role,
        BundleNode::from_projection(&neighborhood.root),
        neighborhood
            .neighbors
            .iter()
            .map(BundleNode::from_projection)
            .collect(),
        neighborhood
            .relations
            .iter()
            .map(BundleRelationship::from_projection)
            .collect(),
        node_details,
        BundleMetadata::initial(generator_version),
    )?)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::sync::Arc;

    use rehydration_domain::{
        ContextPathNeighborhood, NodeDetailProjection, NodeNeighborhood, NodeProjection,
        NodeRelationProjection, PortError, RelationExplanation, RelationSemanticClass,
    };
    use tokio::sync::Mutex;

    use super::NodeCentricProjectionReader;
    use crate::queries::DEFAULT_NATIVE_GRAPH_TRAVERSAL_DEPTH;

    struct StubGraphReader;

    impl rehydration_domain::GraphNeighborhoodReader for StubGraphReader {
        async fn load_neighborhood(
            &self,
            _root_node_id: &str,
            _depth: u32,
        ) -> Result<Option<NodeNeighborhood>, PortError> {
            Ok(Some(NodeNeighborhood {
                root: NodeProjection {
                    node_id: "node-root".to_string(),
                    node_kind: "case".to_string(),
                    title: "Root".to_string(),
                    summary: "Root summary".to_string(),
                    status: "ACTIVE".to_string(),
                    labels: vec!["ProjectionNode".to_string()],
                    properties: BTreeMap::new(),
                    provenance: None,
                },
                neighbors: vec![NodeProjection {
                    node_id: "node-1".to_string(),
                    node_kind: "decision".to_string(),
                    title: "Neighbor".to_string(),
                    summary: "Neighbor summary".to_string(),
                    status: "ACTIVE".to_string(),
                    labels: vec!["ProjectionNode".to_string()],
                    properties: BTreeMap::new(),
                    provenance: None,
                }],
                relations: vec![NodeRelationProjection {
                    source_node_id: "node-root".to_string(),
                    target_node_id: "node-1".to_string(),
                    relation_type: "RELATES_TO".to_string(),
                    explanation: structural_explanation(),
                }],
            }))
        }

        async fn load_context_path(
            &self,
            root_node_id: &str,
            target_node_id: &str,
            _subtree_depth: u32,
        ) -> Result<Option<ContextPathNeighborhood>, PortError> {
            Ok(
                (root_node_id == "node-root" && target_node_id == "node-1").then_some(
                    ContextPathNeighborhood {
                        root: NodeProjection {
                            node_id: "node-root".to_string(),
                            node_kind: "case".to_string(),
                            title: "Root".to_string(),
                            summary: "Root summary".to_string(),
                            status: "ACTIVE".to_string(),
                            labels: vec!["ProjectionNode".to_string()],
                            properties: BTreeMap::new(),
                            provenance: None,
                        },
                        neighbors: vec![
                            NodeProjection {
                                node_id: "node-1".to_string(),
                                node_kind: "decision".to_string(),
                                title: "Neighbor".to_string(),
                                summary: "Neighbor summary".to_string(),
                                status: "ACTIVE".to_string(),
                                labels: vec!["ProjectionNode".to_string()],
                                properties: BTreeMap::new(),
                                provenance: None,
                            },
                            NodeProjection {
                                node_id: "node-2".to_string(),
                                node_kind: "artifact".to_string(),
                                title: "Leaf".to_string(),
                                summary: "Leaf summary".to_string(),
                                status: "READY".to_string(),
                                labels: vec!["ProjectionNode".to_string()],
                                properties: BTreeMap::new(),
                                provenance: None,
                            },
                        ],
                        relations: vec![
                            NodeRelationProjection {
                                source_node_id: "node-root".to_string(),
                                target_node_id: "node-1".to_string(),
                                relation_type: "RELATES_TO".to_string(),
                                explanation: structural_explanation(),
                            },
                            NodeRelationProjection {
                                source_node_id: "node-1".to_string(),
                                target_node_id: "node-2".to_string(),
                                relation_type: "HAS_ARTIFACT".to_string(),
                                explanation: structural_explanation(),
                            },
                        ],
                        path_node_ids: vec!["node-root".to_string(), "node-1".to_string()],
                    },
                ),
            )
        }
    }

    struct StubDetailReader;

    impl rehydration_domain::NodeDetailReader for StubDetailReader {
        async fn load_node_detail(
            &self,
            node_id: &str,
        ) -> Result<Option<NodeDetailProjection>, PortError> {
            Ok((node_id == "node-root").then(|| NodeDetailProjection {
                node_id: node_id.to_string(),
                detail: "Expanded detail".to_string(),
                content_hash: "hash-1".to_string(),
                revision: 2,
            }))
        }
    }

    #[tokio::test]
    async fn load_bundle_returns_graph_native_bundle() {
        let reader = NodeCentricProjectionReader::new(StubGraphReader, StubDetailReader);
        let bundle = reader
            .load_bundle("node-root", "developer", "0.1.0")
            .await
            .expect("bundle load should succeed")
            .expect("bundle should exist");

        assert_eq!(bundle.root_node().node_id(), "node-root");
        assert_eq!(bundle.neighbor_nodes().len(), 1);
        assert_eq!(bundle.relationships().len(), 1);
        assert_eq!(bundle.node_details().len(), 1);
    }

    struct RecordingGraphReader {
        depths: Arc<Mutex<Vec<u32>>>,
    }

    impl rehydration_domain::GraphNeighborhoodReader for RecordingGraphReader {
        async fn load_neighborhood(
            &self,
            _root_node_id: &str,
            depth: u32,
        ) -> Result<Option<NodeNeighborhood>, PortError> {
            self.depths.lock().await.push(depth);
            Ok(None)
        }

        async fn load_context_path(
            &self,
            _root_node_id: &str,
            _target_node_id: &str,
            subtree_depth: u32,
        ) -> Result<Option<ContextPathNeighborhood>, PortError> {
            self.depths.lock().await.push(subtree_depth);
            Ok(None)
        }
    }

    #[tokio::test]
    async fn load_bundle_uses_default_graph_traversal_depth() {
        let depths = Arc::new(Mutex::new(Vec::new()));
        let reader = NodeCentricProjectionReader::new(
            RecordingGraphReader {
                depths: Arc::clone(&depths),
            },
            StubDetailReader,
        );

        let bundle = reader
            .load_bundle("node-root", "developer", "0.1.0")
            .await
            .expect("bundle load should succeed");

        assert!(bundle.is_none());
        assert_eq!(
            &*depths.lock().await,
            &[DEFAULT_NATIVE_GRAPH_TRAVERSAL_DEPTH]
        );
    }

    #[tokio::test]
    async fn load_context_path_bundle_only_includes_details_for_path_nodes() {
        let reader = NodeCentricProjectionReader::new(StubGraphReader, StubDetailReader);
        let bundle = reader
            .load_context_path_bundle_with_depth("node-root", "node-1", "developer", "0.1.0", 4)
            .await
            .expect("path bundle load should succeed")
            .expect("bundle should exist");

        assert_eq!(bundle.root_node().node_id(), "node-root");
        assert_eq!(bundle.neighbor_nodes().len(), 2);
        assert_eq!(bundle.relationships().len(), 2);
        assert_eq!(
            bundle
                .node_details()
                .iter()
                .map(|detail| detail.node_id())
                .collect::<Vec<_>>(),
            vec!["node-root"]
        );
    }

    fn structural_explanation() -> RelationExplanation {
        RelationExplanation::new(RelationSemanticClass::Structural)
    }
}
