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
