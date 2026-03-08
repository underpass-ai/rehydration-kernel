use std::time::SystemTime;

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
