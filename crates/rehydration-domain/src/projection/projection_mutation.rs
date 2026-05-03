use crate::{NodeDetailProjection, NodeProjection, NodeRelationProjection};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProjectionMutation {
    EnsureNode(NodeProjection),
    UpsertNode(NodeProjection),
    UpsertNodeRelation(NodeRelationProjection),
    UpsertNodeDetail(NodeDetailProjection),
}
