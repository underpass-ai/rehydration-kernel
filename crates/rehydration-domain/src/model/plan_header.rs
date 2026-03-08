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
