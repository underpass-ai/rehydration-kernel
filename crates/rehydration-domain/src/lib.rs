pub mod error;
pub mod model;
pub mod projection;
pub mod repositories;
pub mod value_objects;

pub use error::DomainError;
pub use model::{
    CaseHeader, Decision, DecisionRelation, Milestone, PlanHeader, RehydrationBundle,
    RoleContextPack, TaskImpact, WorkItem,
};
pub use projection::{
    NodeDetailProjection, NodeNeighborhood, NodeProjection, NodeRelationProjection,
    ProjectionCheckpoint, ProjectionMutation,
};
pub use repositories::{
    GraphNeighborhoodReader, NodeDetailReader, PortError, ProcessedEventStore,
    ProjectionCheckpointStore, ProjectionWriter, SnapshotStore,
};
pub use value_objects::{BundleMetadata, CaseId, Role};

#[cfg(test)]
mod tests {
    use super::{
        BundleMetadata, CaseHeader, CaseId, Decision, DecisionRelation, DomainError, Milestone,
        PlanHeader, RehydrationBundle, Role, RoleContextPack, TaskImpact, WorkItem,
    };

    #[test]
    fn case_id_requires_a_value() {
        let error = CaseId::new("   ").expect_err("empty case id must fail");
        assert_eq!(error, DomainError::EmptyValue("case_id"));
    }

    #[test]
    fn bundle_keeps_explicit_structured_sections() {
        let case_id = CaseId::new("case-123").expect("case id is valid");
        let role = Role::new("developer").expect("role is valid");
        let pack = RoleContextPack::new(
            role.clone(),
            CaseHeader::new(
                case_id.clone(),
                "Case 123",
                "Delivery planning",
                "ACTIVE",
                std::time::SystemTime::UNIX_EPOCH,
                "planner",
            ),
            Some(PlanHeader::new("plan-123", 3, "ACTIVE", 1, 0)),
            vec![WorkItem::new(
                "task-1",
                "Implement projection model",
                "Add structured pack support",
                role.as_str(),
                "PHASE_BUILD",
                "READY",
                Vec::new(),
                1,
            )],
            vec![Decision::new(
                "decision-1",
                "Adopt projection packs",
                "Stop reading pre-rendered bundles from infrastructure",
                "ACCEPTED",
                "platform",
                std::time::SystemTime::UNIX_EPOCH,
            )],
            vec![DecisionRelation::new(
                "decision-1",
                "decision-2",
                "INFLUENCES",
            )],
            vec![TaskImpact::new(
                "decision-1",
                "task-1",
                "Transport mapping must stop inventing work items",
                "DIRECT",
            )],
            vec![Milestone::new(
                "PHASE_TRANSITIONED",
                "Moved from planning to build",
                std::time::SystemTime::UNIX_EPOCH,
                "system",
            )],
            "Projection snapshot loaded",
            4096,
        );
        let bundle = RehydrationBundle::new(
            pack,
            vec![
                "Projection snapshot loaded".to_string(),
                "Implement projection model: Add structured pack support".to_string(),
            ],
            BundleMetadata::initial("0.1.0"),
        );

        assert_eq!(bundle.case_id().as_str(), "case-123");
        assert_eq!(bundle.role().as_str(), "developer");
        assert_eq!(bundle.sections()[0], "Projection snapshot loaded");
    }
}
