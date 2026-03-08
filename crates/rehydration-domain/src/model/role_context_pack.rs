use crate::{
    CaseHeader, Decision, DecisionRelation, Milestone, PlanHeader, Role, TaskImpact, WorkItem,
};

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
