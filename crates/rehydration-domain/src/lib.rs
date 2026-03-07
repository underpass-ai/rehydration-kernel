use std::error::Error;
use std::fmt;
use std::time::SystemTime;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DomainError {
    EmptyValue(&'static str),
}

impl fmt::Display for DomainError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyValue(field) => write!(f, "{field} cannot be empty"),
        }
    }
}

impl Error for DomainError {}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CaseId(String);

impl CaseId {
    pub fn new(value: impl Into<String>) -> Result<Self, DomainError> {
        let value = value.into();
        if value.trim().is_empty() {
            return Err(DomainError::EmptyValue("case_id"));
        }

        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Role(String);

impl Role {
    pub fn new(value: impl Into<String>) -> Result<Self, DomainError> {
        let value = value.into();
        if value.trim().is_empty() {
            return Err(DomainError::EmptyValue("role"));
        }

        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BundleMetadata {
    pub revision: u64,
    pub content_hash: String,
    pub generator_version: String,
}

impl BundleMetadata {
    pub fn initial(generator_version: impl Into<String>) -> Self {
        Self {
            revision: 1,
            content_hash: "pending".to_string(),
            generator_version: generator_version.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaseHeader {
    case_id: CaseId,
    title: String,
    summary: String,
    status: String,
    created_at: SystemTime,
    created_by: String,
}

impl CaseHeader {
    pub fn new(
        case_id: CaseId,
        title: impl Into<String>,
        summary: impl Into<String>,
        status: impl Into<String>,
        created_at: SystemTime,
        created_by: impl Into<String>,
    ) -> Self {
        Self {
            case_id,
            title: title.into(),
            summary: summary.into(),
            status: status.into(),
            created_at,
            created_by: created_by.into(),
        }
    }

    pub fn case_id(&self) -> &CaseId {
        &self.case_id
    }

    pub fn title(&self) -> &str {
        &self.title
    }

    pub fn summary(&self) -> &str {
        &self.summary
    }

    pub fn status(&self) -> &str {
        &self.status
    }

    pub fn created_at(&self) -> SystemTime {
        self.created_at
    }

    pub fn created_by(&self) -> &str {
        &self.created_by
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanHeader {
    plan_id: String,
    revision: u64,
    status: String,
    work_items_total: u32,
    work_items_completed: u32,
}

impl PlanHeader {
    pub fn new(
        plan_id: impl Into<String>,
        revision: u64,
        status: impl Into<String>,
        work_items_total: u32,
        work_items_completed: u32,
    ) -> Self {
        Self {
            plan_id: plan_id.into(),
            revision,
            status: status.into(),
            work_items_total,
            work_items_completed,
        }
    }

    pub fn plan_id(&self) -> &str {
        &self.plan_id
    }

    pub fn revision(&self) -> u64 {
        self.revision
    }

    pub fn status(&self) -> &str {
        &self.status
    }

    pub fn work_items_total(&self) -> u32 {
        self.work_items_total
    }

    pub fn work_items_completed(&self) -> u32 {
        self.work_items_completed
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkItem {
    work_item_id: String,
    title: String,
    summary: String,
    role: String,
    phase: String,
    status: String,
    dependency_ids: Vec<String>,
    priority: u32,
}

impl WorkItem {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        work_item_id: impl Into<String>,
        title: impl Into<String>,
        summary: impl Into<String>,
        role: impl Into<String>,
        phase: impl Into<String>,
        status: impl Into<String>,
        dependency_ids: Vec<String>,
        priority: u32,
    ) -> Self {
        Self {
            work_item_id: work_item_id.into(),
            title: title.into(),
            summary: summary.into(),
            role: role.into(),
            phase: phase.into(),
            status: status.into(),
            dependency_ids,
            priority,
        }
    }

    pub fn work_item_id(&self) -> &str {
        &self.work_item_id
    }

    pub fn title(&self) -> &str {
        &self.title
    }

    pub fn summary(&self) -> &str {
        &self.summary
    }

    pub fn role(&self) -> &str {
        &self.role
    }

    pub fn phase(&self) -> &str {
        &self.phase
    }

    pub fn status(&self) -> &str {
        &self.status
    }

    pub fn dependency_ids(&self) -> &[String] {
        &self.dependency_ids
    }

    pub fn priority(&self) -> u32 {
        self.priority
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Decision {
    decision_id: String,
    title: String,
    rationale: String,
    status: String,
    owner: String,
    decided_at: SystemTime,
}

impl Decision {
    pub fn new(
        decision_id: impl Into<String>,
        title: impl Into<String>,
        rationale: impl Into<String>,
        status: impl Into<String>,
        owner: impl Into<String>,
        decided_at: SystemTime,
    ) -> Self {
        Self {
            decision_id: decision_id.into(),
            title: title.into(),
            rationale: rationale.into(),
            status: status.into(),
            owner: owner.into(),
            decided_at,
        }
    }

    pub fn decision_id(&self) -> &str {
        &self.decision_id
    }

    pub fn title(&self) -> &str {
        &self.title
    }

    pub fn rationale(&self) -> &str {
        &self.rationale
    }

    pub fn status(&self) -> &str {
        &self.status
    }

    pub fn owner(&self) -> &str {
        &self.owner
    }

    pub fn decided_at(&self) -> SystemTime {
        self.decided_at
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecisionRelation {
    source_decision_id: String,
    target_decision_id: String,
    relation_type: String,
}

impl DecisionRelation {
    pub fn new(
        source_decision_id: impl Into<String>,
        target_decision_id: impl Into<String>,
        relation_type: impl Into<String>,
    ) -> Self {
        Self {
            source_decision_id: source_decision_id.into(),
            target_decision_id: target_decision_id.into(),
            relation_type: relation_type.into(),
        }
    }

    pub fn source_decision_id(&self) -> &str {
        &self.source_decision_id
    }

    pub fn target_decision_id(&self) -> &str {
        &self.target_decision_id
    }

    pub fn relation_type(&self) -> &str {
        &self.relation_type
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskImpact {
    decision_id: String,
    work_item_id: String,
    title: String,
    impact_type: String,
}

impl TaskImpact {
    pub fn new(
        decision_id: impl Into<String>,
        work_item_id: impl Into<String>,
        title: impl Into<String>,
        impact_type: impl Into<String>,
    ) -> Self {
        Self {
            decision_id: decision_id.into(),
            work_item_id: work_item_id.into(),
            title: title.into(),
            impact_type: impact_type.into(),
        }
    }

    pub fn decision_id(&self) -> &str {
        &self.decision_id
    }

    pub fn work_item_id(&self) -> &str {
        &self.work_item_id
    }

    pub fn title(&self) -> &str {
        &self.title
    }

    pub fn impact_type(&self) -> &str {
        &self.impact_type
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Milestone {
    milestone_type: String,
    description: String,
    occurred_at: SystemTime,
    actor: String,
}

impl Milestone {
    pub fn new(
        milestone_type: impl Into<String>,
        description: impl Into<String>,
        occurred_at: SystemTime,
        actor: impl Into<String>,
    ) -> Self {
        Self {
            milestone_type: milestone_type.into(),
            description: description.into(),
            occurred_at,
            actor: actor.into(),
        }
    }

    pub fn milestone_type(&self) -> &str {
        &self.milestone_type
    }

    pub fn description(&self) -> &str {
        &self.description
    }

    pub fn occurred_at(&self) -> SystemTime {
        self.occurred_at
    }

    pub fn actor(&self) -> &str {
        &self.actor
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoleContextPack {
    role: Role,
    case_header: CaseHeader,
    plan_header: Option<PlanHeader>,
    work_items: Vec<WorkItem>,
    decisions: Vec<Decision>,
    decision_relations: Vec<DecisionRelation>,
    impacts: Vec<TaskImpact>,
    milestones: Vec<Milestone>,
    latest_summary: String,
    token_budget_hint: u32,
}

impl RoleContextPack {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        role: Role,
        case_header: CaseHeader,
        plan_header: Option<PlanHeader>,
        work_items: Vec<WorkItem>,
        decisions: Vec<Decision>,
        decision_relations: Vec<DecisionRelation>,
        impacts: Vec<TaskImpact>,
        milestones: Vec<Milestone>,
        latest_summary: impl Into<String>,
        token_budget_hint: u32,
    ) -> Self {
        Self {
            role,
            case_header,
            plan_header,
            work_items,
            decisions,
            decision_relations,
            impacts,
            milestones,
            latest_summary: latest_summary.into(),
            token_budget_hint,
        }
    }

    pub fn placeholder(case_id: CaseId, role: Role) -> Self {
        let summary = format!(
            "bundle for case {} role {}",
            case_id.as_str(),
            role.as_str()
        );

        Self::new(
            role,
            CaseHeader::new(
                case_id.clone(),
                format!("Case {}", case_id.as_str()),
                summary.clone(),
                "ACTIVE",
                SystemTime::UNIX_EPOCH,
                "rehydration-kernel",
            ),
            Some(PlanHeader::new(
                format!("plan:{}", case_id.as_str()),
                1,
                "PENDING",
                0,
                0,
            )),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            summary,
            4096,
        )
    }

    pub fn role(&self) -> &Role {
        &self.role
    }

    pub fn case_header(&self) -> &CaseHeader {
        &self.case_header
    }

    pub fn plan_header(&self) -> Option<&PlanHeader> {
        self.plan_header.as_ref()
    }

    pub fn work_items(&self) -> &[WorkItem] {
        &self.work_items
    }

    pub fn decisions(&self) -> &[Decision] {
        &self.decisions
    }

    pub fn decision_relations(&self) -> &[DecisionRelation] {
        &self.decision_relations
    }

    pub fn impacts(&self) -> &[TaskImpact] {
        &self.impacts
    }

    pub fn milestones(&self) -> &[Milestone] {
        &self.milestones
    }

    pub fn latest_summary(&self) -> &str {
        &self.latest_summary
    }

    pub fn token_budget_hint(&self) -> u32 {
        self.token_budget_hint
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RehydrationBundle {
    case_id: CaseId,
    pack: RoleContextPack,
    sections: Vec<String>,
    metadata: BundleMetadata,
}

impl RehydrationBundle {
    pub fn new(
        case_id: CaseId,
        role: Role,
        sections: Vec<String>,
        metadata: BundleMetadata,
    ) -> Self {
        let pack = RoleContextPack::new(
            role.clone(),
            CaseHeader::new(
                case_id.clone(),
                format!("Case {}", case_id.as_str()),
                sections.first().cloned().unwrap_or_else(|| {
                    format!(
                        "bundle for case {} role {}",
                        case_id.as_str(),
                        role.as_str()
                    )
                }),
                "ACTIVE",
                SystemTime::UNIX_EPOCH,
                "rehydration-kernel",
            ),
            Some(PlanHeader::new(
                format!("plan:{}", case_id.as_str()),
                metadata.revision,
                "PLACEHOLDER",
                sections.len() as u32,
                0,
            )),
            sections
                .iter()
                .enumerate()
                .map(|(index, section)| {
                    WorkItem::new(
                        format!("section-{index}"),
                        format!("Section {}", index + 1),
                        section.clone(),
                        role.as_str(),
                        "PHASE_BUILD",
                        "READY",
                        Vec::new(),
                        (index + 1) as u32,
                    )
                })
                .collect(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            sections.join(" "),
            4096,
        );

        Self {
            case_id,
            pack,
            sections,
            metadata,
        }
    }

    pub fn from_pack(pack: RoleContextPack, metadata: BundleMetadata) -> Self {
        let case_id = pack.case_header().case_id().clone();
        let sections = sections_from_pack(&pack);

        Self {
            case_id,
            pack,
            sections,
            metadata,
        }
    }

    pub fn empty(case_id: CaseId, role: Role, generator_version: &str) -> Self {
        Self::from_pack(
            RoleContextPack::placeholder(case_id, role),
            BundleMetadata::initial(generator_version),
        )
    }

    pub fn case_id(&self) -> &CaseId {
        &self.case_id
    }

    pub fn role(&self) -> &Role {
        self.pack.role()
    }

    pub fn pack(&self) -> &RoleContextPack {
        &self.pack
    }

    pub fn sections(&self) -> &[String] {
        &self.sections
    }

    pub fn metadata(&self) -> &BundleMetadata {
        &self.metadata
    }
}

fn sections_from_pack(pack: &RoleContextPack) -> Vec<String> {
    let mut sections = Vec::new();

    if !pack.latest_summary().trim().is_empty() {
        sections.push(pack.latest_summary().to_string());
    }

    sections.extend(pack.work_items().iter().filter_map(|work_item| {
        let title = work_item.title().trim();
        let summary = work_item.summary().trim();
        match (title.is_empty(), summary.is_empty()) {
            (true, false) => Some(summary.to_string()),
            (false, false) => Some(format!("{title}: {summary}")),
            (false, true) => Some(title.to_string()),
            (true, true) => None,
        }
    }));

    sections.extend(pack.decisions().iter().filter_map(|decision| {
        let title = decision.title().trim();
        let rationale = decision.rationale().trim();
        match (title.is_empty(), rationale.is_empty()) {
            (true, false) => Some(format!("Decision: {rationale}")),
            (false, false) => Some(format!("Decision {title}: {rationale}")),
            (false, true) => Some(format!("Decision {title}")),
            (true, true) => None,
        }
    }));

    sections.extend(pack.impacts().iter().filter_map(|impact| {
        let title = impact.title().trim();
        if title.is_empty() {
            None
        } else {
            Some(format!("Impact {}: {}", impact.impact_type(), title))
        }
    }));

    sections.extend(pack.milestones().iter().filter_map(|milestone| {
        let description = milestone.description().trim();
        if description.is_empty() {
            None
        } else {
            Some(format!(
                "Milestone {}: {}",
                milestone.milestone_type(),
                description
            ))
        }
    }));

    if sections.is_empty() {
        let summary = pack.case_header().summary().trim();
        if !summary.is_empty() {
            sections.push(summary.to_string());
        }
    }

    sections
}

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
    fn empty_bundle_uses_initial_metadata() {
        let bundle = RehydrationBundle::empty(
            CaseId::new("case-123").expect("case id is valid"),
            Role::new("reviewer").expect("role is valid"),
            "0.1.0",
        );

        assert_eq!(bundle.metadata().revision, 1);
        assert_eq!(
            bundle.pack().latest_summary(),
            "bundle for case case-123 role reviewer"
        );
        assert_eq!(
            bundle.sections(),
            &["bundle for case case-123 role reviewer"]
        );
    }

    #[test]
    fn bundle_from_pack_derives_structured_sections() {
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

        let bundle = RehydrationBundle::from_pack(pack, BundleMetadata::initial("0.1.0"));

        assert_eq!(bundle.case_id().as_str(), "case-123");
        assert_eq!(bundle.role().as_str(), "developer");
        assert_eq!(bundle.sections()[0], "Projection snapshot loaded");
        assert!(bundle.sections()[1].contains("Implement projection model"));
        assert!(bundle.sections()[2].contains("Adopt projection packs"));
        assert!(bundle.sections()[3].contains("DIRECT"));
        assert!(bundle.sections()[4].contains("PHASE_TRANSITIONED"));
    }
}
