use std::time::SystemTime;

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
