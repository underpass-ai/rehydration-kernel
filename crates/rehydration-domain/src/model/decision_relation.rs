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
