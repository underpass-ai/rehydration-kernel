use std::collections::BTreeMap;

use crate::{DomainError, RelationSemanticClass};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelationExplanation {
    semantic_class: RelationSemanticClass,
    rationale: Option<String>,
    motivation: Option<String>,
    method: Option<String>,
    decision_id: Option<String>,
    caused_by_node_id: Option<String>,
    evidence: Option<String>,
    confidence: Option<String>,
    sequence: Option<u32>,
}

impl RelationExplanation {
    pub fn new(semantic_class: RelationSemanticClass) -> Self {
        Self {
            semantic_class,
            rationale: None,
            motivation: None,
            method: None,
            decision_id: None,
            caused_by_node_id: None,
            evidence: None,
            confidence: None,
            sequence: None,
        }
    }

    pub fn from_properties(properties: &BTreeMap<String, String>) -> Result<Self, DomainError> {
        let semantic_class =
            RelationSemanticClass::parse(properties.get("semantic_class").ok_or_else(|| {
                DomainError::InvalidState(
                    "relation explanation is missing `semantic_class`".to_string(),
                )
            })?)?;

        Ok(Self::new(semantic_class)
            .with_optional_rationale(properties.get("rationale").cloned())
            .with_optional_motivation(properties.get("motivation").cloned())
            .with_optional_method(properties.get("method").cloned())
            .with_optional_decision_id(properties.get("decision_id").cloned())
            .with_optional_caused_by_node_id(properties.get("caused_by_node_id").cloned())
            .with_optional_evidence(properties.get("evidence").cloned())
            .with_optional_confidence(properties.get("confidence").cloned())
            .with_optional_sequence(
                properties
                    .get("sequence")
                    .or_else(|| properties.get("order"))
                    .map(|value| {
                        value.parse::<u32>().map_err(|error| {
                            DomainError::InvalidState(format!(
                                "invalid relation sequence `{value}`: {error}"
                            ))
                        })
                    })
                    .transpose()?,
            ))
    }

    pub fn to_properties(&self) -> BTreeMap<String, String> {
        let mut properties = BTreeMap::new();

        properties.insert(
            "semantic_class".to_string(),
            self.semantic_class.as_str().to_string(),
        );
        insert_optional(&mut properties, "rationale", self.rationale.as_deref());
        insert_optional(&mut properties, "motivation", self.motivation.as_deref());
        insert_optional(&mut properties, "method", self.method.as_deref());
        insert_optional(&mut properties, "decision_id", self.decision_id.as_deref());
        insert_optional(
            &mut properties,
            "caused_by_node_id",
            self.caused_by_node_id.as_deref(),
        );
        insert_optional(&mut properties, "evidence", self.evidence.as_deref());
        insert_optional(&mut properties, "confidence", self.confidence.as_deref());
        if let Some(sequence) = self.sequence {
            properties.insert("sequence".to_string(), sequence.to_string());
        }

        properties
    }

    pub fn semantic_class(&self) -> &RelationSemanticClass {
        &self.semantic_class
    }

    pub fn rationale(&self) -> Option<&str> {
        self.rationale.as_deref()
    }

    pub fn motivation(&self) -> Option<&str> {
        self.motivation.as_deref()
    }

    pub fn method(&self) -> Option<&str> {
        self.method.as_deref()
    }

    pub fn decision_id(&self) -> Option<&str> {
        self.decision_id.as_deref()
    }

    pub fn caused_by_node_id(&self) -> Option<&str> {
        self.caused_by_node_id.as_deref()
    }

    pub fn evidence(&self) -> Option<&str> {
        self.evidence.as_deref()
    }

    pub fn confidence(&self) -> Option<&str> {
        self.confidence.as_deref()
    }

    pub fn sequence(&self) -> Option<u32> {
        self.sequence
    }

    pub fn with_rationale(mut self, value: impl Into<String>) -> Self {
        self.rationale = normalize_string(Some(value.into()));
        self
    }

    pub fn with_optional_rationale(mut self, value: Option<String>) -> Self {
        self.rationale = normalize_string(value);
        self
    }

    pub fn with_motivation(mut self, value: impl Into<String>) -> Self {
        self.motivation = normalize_string(Some(value.into()));
        self
    }

    pub fn with_optional_motivation(mut self, value: Option<String>) -> Self {
        self.motivation = normalize_string(value);
        self
    }

    pub fn with_method(mut self, value: impl Into<String>) -> Self {
        self.method = normalize_string(Some(value.into()));
        self
    }

    pub fn with_optional_method(mut self, value: Option<String>) -> Self {
        self.method = normalize_string(value);
        self
    }

    pub fn with_decision_id(mut self, value: impl Into<String>) -> Self {
        self.decision_id = normalize_string(Some(value.into()));
        self
    }

    pub fn with_optional_decision_id(mut self, value: Option<String>) -> Self {
        self.decision_id = normalize_string(value);
        self
    }

    pub fn with_caused_by_node_id(mut self, value: impl Into<String>) -> Self {
        self.caused_by_node_id = normalize_string(Some(value.into()));
        self
    }

    pub fn with_optional_caused_by_node_id(mut self, value: Option<String>) -> Self {
        self.caused_by_node_id = normalize_string(value);
        self
    }

    pub fn with_evidence(mut self, value: impl Into<String>) -> Self {
        self.evidence = normalize_string(Some(value.into()));
        self
    }

    pub fn with_optional_evidence(mut self, value: Option<String>) -> Self {
        self.evidence = normalize_string(value);
        self
    }

    pub fn with_confidence(mut self, value: impl Into<String>) -> Self {
        self.confidence = normalize_string(Some(value.into()));
        self
    }

    pub fn with_optional_confidence(mut self, value: Option<String>) -> Self {
        self.confidence = normalize_string(value);
        self
    }

    pub fn with_sequence(mut self, value: u32) -> Self {
        self.sequence = Some(value);
        self
    }

    pub fn with_optional_sequence(mut self, value: Option<u32>) -> Self {
        self.sequence = value;
        self
    }
}

fn normalize_string(value: Option<String>) -> Option<String> {
    let value = value?;
    let trimmed = value.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_string())
}

fn insert_optional(properties: &mut BTreeMap<String, String>, key: &str, value: Option<&str>) {
    if let Some(value) = value {
        properties.insert(key.to_string(), value.to_string());
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use crate::{RelationExplanation, RelationSemanticClass};

    #[test]
    fn explanation_roundtrip_preserves_typed_fields() {
        let explanation = RelationExplanation::new(RelationSemanticClass::Motivational)
            .with_rationale("reserve power must be diverted before repair")
            .with_decision_id("decision-1")
            .with_sequence(2);

        let properties = explanation.to_properties();
        let reparsed =
            RelationExplanation::from_properties(&properties).expect("properties should parse");

        assert_eq!(
            reparsed.semantic_class(),
            &RelationSemanticClass::Motivational
        );
        assert_eq!(
            reparsed.rationale(),
            Some("reserve power must be diverted before repair")
        );
        assert_eq!(reparsed.decision_id(), Some("decision-1"));
        assert_eq!(reparsed.sequence(), Some(2));
    }

    #[test]
    fn explanation_requires_semantic_class() {
        let error = RelationExplanation::from_properties(&BTreeMap::new())
            .expect_err("missing semantic class must fail");

        assert_eq!(
            error,
            crate::DomainError::InvalidState(
                "relation explanation is missing `semantic_class`".to_string()
            )
        );
    }
}
