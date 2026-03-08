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
