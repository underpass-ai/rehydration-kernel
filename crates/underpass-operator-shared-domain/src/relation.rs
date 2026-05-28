const STRUCTURAL_CLASSES: &[RelationSemanticClass] = &[RelationSemanticClass::Structural];
const CAUSAL_CLASSES: &[RelationSemanticClass] = &[RelationSemanticClass::Causal];
const MOTIVATIONAL_OR_CAUSAL_CLASSES: &[RelationSemanticClass] = &[
    RelationSemanticClass::Causal,
    RelationSemanticClass::Motivational,
];
const PROCEDURAL_CLASSES: &[RelationSemanticClass] = &[RelationSemanticClass::Procedural];
const EVIDENTIAL_CLASSES: &[RelationSemanticClass] = &[RelationSemanticClass::Evidential];
const EVIDENTIAL_OR_MOTIVATIONAL_CLASSES: &[RelationSemanticClass] = &[
    RelationSemanticClass::Evidential,
    RelationSemanticClass::Motivational,
];
const CONSTRAINT_CLASSES: &[RelationSemanticClass] = &[RelationSemanticClass::Constraint];
const WRITER_RELATION_TYPES: &[KnownMemoryRelationType] = &[
    KnownMemoryRelationType::Follows,
    KnownMemoryRelationType::Answers,
    KnownMemoryRelationType::UsesBackground,
    KnownMemoryRelationType::DependsOn,
    KnownMemoryRelationType::ChosenBecause,
    KnownMemoryRelationType::SemanticDeltaFrom,
    KnownMemoryRelationType::UpdatesState,
    KnownMemoryRelationType::Supports,
    KnownMemoryRelationType::Supersedes,
    KnownMemoryRelationType::Contradicts,
    KnownMemoryRelationType::SatisfiesConstraint,
    KnownMemoryRelationType::ViolatesConstraint,
    KnownMemoryRelationType::ContributesTo,
    KnownMemoryRelationType::ExcludedFrom,
    KnownMemoryRelationType::CheckedAgainst,
    KnownMemoryRelationType::DerivedFrom,
    KnownMemoryRelationType::ConfirmsSelection,
    KnownMemoryRelationType::Restates,
    KnownMemoryRelationType::Corrects,
    KnownMemoryRelationType::ComponentOf,
    KnownMemoryRelationType::TotalOf,
    KnownMemoryRelationType::SameEventAs,
    KnownMemoryRelationType::SameEntityAs,
    KnownMemoryRelationType::QualifiesAs,
    KnownMemoryRelationType::MatchesRequirement,
    KnownMemoryRelationType::Contains,
    KnownMemoryRelationType::MemberOf,
    KnownMemoryRelationType::ScopedTo,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RelationSemanticClass {
    Structural,
    Causal,
    Motivational,
    Procedural,
    Evidential,
    Constraint,
}

impl RelationSemanticClass {
    pub fn parse(value: &str) -> Result<Self, String> {
        match value {
            "structural" => Ok(Self::Structural),
            "causal" => Ok(Self::Causal),
            "motivational" => Ok(Self::Motivational),
            "procedural" => Ok(Self::Procedural),
            "evidential" => Ok(Self::Evidential),
            "constraint" => Ok(Self::Constraint),
            other => Err(format!("invalid relation semantic_class `{other}`")),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Structural => "structural",
            Self::Causal => "causal",
            Self::Motivational => "motivational",
            Self::Procedural => "procedural",
            Self::Evidential => "evidential",
            Self::Constraint => "constraint",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct MemoryRelationType {
    canonical: String,
    known: Option<KnownMemoryRelationType>,
}

impl MemoryRelationType {
    pub fn new(value: impl AsRef<str>) -> Result<Self, String> {
        let trimmed = value.as_ref().trim();
        if trimmed.is_empty() {
            return Err("relation_type must not be empty".to_string());
        }
        let known = KnownMemoryRelationType::parse(trimmed);
        let canonical = known
            .map(|known| known.as_str().to_string())
            .unwrap_or_else(|| trimmed.to_string());
        Ok(Self { canonical, known })
    }

    pub fn as_str(&self) -> &str {
        &self.canonical
    }

    pub fn known(&self) -> Option<KnownMemoryRelationType> {
        self.known
    }

    pub fn writer_spec(&self) -> Option<MemoryRelationSpec> {
        self.known.and_then(KnownMemoryRelationType::writer_spec)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum KnownMemoryRelationType {
    Contains,
    ContainsEntry,
    HasDimension,
    HasEvidence,
    MemberOf,
    Records,
    ScopedTo,
    Follows,
    Answers,
    UsesBackground,
    DependsOn,
    ChosenBecause,
    SemanticDeltaFrom,
    UpdatesState,
    Supports,
    SupportsAnswer,
    Supersedes,
    Contradicts,
    ConfirmsSelection,
    SatisfiesConstraint,
    ViolatesConstraint,
    ContributesTo,
    ExcludedFrom,
    CheckedAgainst,
    DerivedFrom,
    Restates,
    Corrects,
    ComponentOf,
    TotalOf,
    SameEventAs,
    SameEntityAs,
    QualifiesAs,
    MatchesRequirement,
}

impl KnownMemoryRelationType {
    pub fn parse(value: &str) -> Option<Self> {
        match normalize_relation_type(value).as_str() {
            "contains" => Some(Self::Contains),
            "contains_entry" => Some(Self::ContainsEntry),
            "has_dimension" => Some(Self::HasDimension),
            "has_evidence" => Some(Self::HasEvidence),
            "member_of" => Some(Self::MemberOf),
            "records" => Some(Self::Records),
            "scoped_to" => Some(Self::ScopedTo),
            "follows" => Some(Self::Follows),
            "answers" => Some(Self::Answers),
            "uses_background" => Some(Self::UsesBackground),
            "depends_on" => Some(Self::DependsOn),
            "chosen_because" => Some(Self::ChosenBecause),
            "semantic_delta_from" => Some(Self::SemanticDeltaFrom),
            "updates_state" => Some(Self::UpdatesState),
            "supports" => Some(Self::Supports),
            "supports_answer" => Some(Self::SupportsAnswer),
            "supersedes" => Some(Self::Supersedes),
            "contradicts" | "conflicts" | "conflicts_with" | "conflict_with" => {
                Some(Self::Contradicts)
            }
            "confirms_selection" => Some(Self::ConfirmsSelection),
            "satisfies_constraint" => Some(Self::SatisfiesConstraint),
            "violates_constraint" => Some(Self::ViolatesConstraint),
            "contributes_to" => Some(Self::ContributesTo),
            "excluded_from" => Some(Self::ExcludedFrom),
            "checked_against" => Some(Self::CheckedAgainst),
            "derived_from" => Some(Self::DerivedFrom),
            "restates" => Some(Self::Restates),
            "corrects" => Some(Self::Corrects),
            "component_of" => Some(Self::ComponentOf),
            "total_of" => Some(Self::TotalOf),
            "same_event_as" => Some(Self::SameEventAs),
            "same_entity_as" => Some(Self::SameEntityAs),
            "qualifies_as" => Some(Self::QualifiesAs),
            "matches_requirement" | "matches_question_item" => Some(Self::MatchesRequirement),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Contains => "contains",
            Self::ContainsEntry => "contains_entry",
            Self::HasDimension => "has_dimension",
            Self::HasEvidence => "has_evidence",
            Self::MemberOf => "member_of",
            Self::Records => "records",
            Self::ScopedTo => "scoped_to",
            Self::Follows => "follows",
            Self::Answers => "answers",
            Self::UsesBackground => "uses_background",
            Self::DependsOn => "depends_on",
            Self::ChosenBecause => "chosen_because",
            Self::SemanticDeltaFrom => "semantic_delta_from",
            Self::UpdatesState => "updates_state",
            Self::Supports => "supports",
            Self::SupportsAnswer => "supports_answer",
            Self::Supersedes => "supersedes",
            Self::Contradicts => "contradicts",
            Self::ConfirmsSelection => "confirms_selection",
            Self::SatisfiesConstraint => "satisfies_constraint",
            Self::ViolatesConstraint => "violates_constraint",
            Self::ContributesTo => "contributes_to",
            Self::ExcludedFrom => "excluded_from",
            Self::CheckedAgainst => "checked_against",
            Self::DerivedFrom => "derived_from",
            Self::Restates => "restates",
            Self::Corrects => "corrects",
            Self::ComponentOf => "component_of",
            Self::TotalOf => "total_of",
            Self::SameEventAs => "same_event_as",
            Self::SameEntityAs => "same_entity_as",
            Self::QualifiesAs => "qualifies_as",
            Self::MatchesRequirement => "matches_requirement",
        }
    }

    pub fn writer_spec(self) -> Option<MemoryRelationSpec> {
        match self {
            Self::Follows => Some(MemoryRelationSpec::new(
                self,
                MemoryRelationQuality::Anemic,
                PROCEDURAL_CLASSES,
            )),
            Self::Answers | Self::UsesBackground => Some(MemoryRelationSpec::new(
                self,
                MemoryRelationQuality::Anemic,
                EVIDENTIAL_CLASSES,
            )),
            Self::DependsOn | Self::SemanticDeltaFrom | Self::UpdatesState => Some(
                MemoryRelationSpec::new(self, MemoryRelationQuality::Rich, CAUSAL_CLASSES),
            ),
            Self::ChosenBecause => Some(MemoryRelationSpec::new(
                self,
                MemoryRelationQuality::Rich,
                MOTIVATIONAL_OR_CAUSAL_CLASSES,
            )),
            Self::Supports
            | Self::Supersedes
            | Self::Contradicts
            | Self::ContributesTo
            | Self::DerivedFrom
            | Self::Restates
            | Self::Corrects
            | Self::ComponentOf
            | Self::TotalOf
            | Self::SameEventAs
            | Self::SameEntityAs
            | Self::QualifiesAs => Some(MemoryRelationSpec::new(
                self,
                MemoryRelationQuality::Rich,
                EVIDENTIAL_CLASSES,
            )),
            Self::ConfirmsSelection => Some(MemoryRelationSpec::new(
                self,
                MemoryRelationQuality::Rich,
                EVIDENTIAL_OR_MOTIVATIONAL_CLASSES,
            )),
            Self::SatisfiesConstraint
            | Self::ViolatesConstraint
            | Self::ExcludedFrom
            | Self::CheckedAgainst
            | Self::MatchesRequirement => Some(MemoryRelationSpec::new(
                self,
                MemoryRelationQuality::Rich,
                CONSTRAINT_CLASSES,
            )),
            Self::Contains | Self::MemberOf | Self::ScopedTo => Some(MemoryRelationSpec::new(
                self,
                MemoryRelationQuality::Structural,
                STRUCTURAL_CLASSES,
            )),
            Self::ContainsEntry
            | Self::HasDimension
            | Self::HasEvidence
            | Self::Records
            | Self::SupportsAnswer => None,
        }
    }

    pub fn writer_relation_types() -> &'static [Self] {
        WRITER_RELATION_TYPES
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MemoryRelationQuality {
    Rich,
    Anemic,
    Structural,
    Suspect,
}

impl MemoryRelationQuality {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Rich => "rich",
            Self::Anemic => "anemic",
            Self::Structural => "structural",
            Self::Suspect => "suspect",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MemoryRelationSpec {
    relation_type: KnownMemoryRelationType,
    quality: MemoryRelationQuality,
    allowed_classes: &'static [RelationSemanticClass],
}

impl MemoryRelationSpec {
    const fn new(
        relation_type: KnownMemoryRelationType,
        quality: MemoryRelationQuality,
        allowed_classes: &'static [RelationSemanticClass],
    ) -> Self {
        Self {
            relation_type,
            quality,
            allowed_classes,
        }
    }

    pub fn relation_type(self) -> KnownMemoryRelationType {
        self.relation_type
    }

    pub fn quality(self) -> MemoryRelationQuality {
        self.quality
    }

    pub fn allowed_classes(self) -> &'static [RelationSemanticClass] {
        self.allowed_classes
    }

    pub fn allows_class(self, semantic_class: &RelationSemanticClass) -> bool {
        self.allowed_classes.contains(semantic_class)
    }
}

fn normalize_relation_type(value: &str) -> String {
    value
        .trim()
        .chars()
        .map(|ch| {
            if ch == '-' || ch == ' ' {
                '_'
            } else {
                ch.to_ascii_lowercase()
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{
        KnownMemoryRelationType, MemoryRelationQuality, MemoryRelationType, RelationSemanticClass,
    };

    #[test]
    fn relation_type_canonicalizes_known_wire_names() {
        let relation = MemoryRelationType::new(" CONFLICTS-WITH ").expect("valid relation");

        assert_eq!(relation.as_str(), "contradicts");
        assert_eq!(relation.known(), Some(KnownMemoryRelationType::Contradicts));
    }

    #[test]
    fn relation_type_preserves_unknown_extensions() {
        let relation = MemoryRelationType::new("ORG_CUSTOM_REL").expect("valid relation");

        assert_eq!(relation.as_str(), "ORG_CUSTOM_REL");
        assert_eq!(relation.known(), None);
        assert_eq!(relation.writer_spec(), None);
    }

    #[test]
    fn writer_relation_aliases_are_canonical() {
        assert_eq!(
            MemoryRelationType::new("matches_question_item")
                .expect("valid relation")
                .as_str(),
            "matches_requirement"
        );
    }

    #[test]
    fn writer_specs_cover_writer_vocabulary() {
        for relation_type in KnownMemoryRelationType::writer_relation_types() {
            assert!(
                relation_type.writer_spec().is_some(),
                "{} must be valid for operator writer",
                relation_type.as_str()
            );
        }
    }

    #[test]
    fn writer_specs_reject_wrong_semantic_classes() {
        let spec = MemoryRelationType::new("contributes_to")
            .expect("valid relation")
            .writer_spec()
            .expect("writer spec");

        assert_eq!(spec.quality(), MemoryRelationQuality::Rich);
        assert!(spec.allows_class(&RelationSemanticClass::Evidential));
        assert!(!spec.allows_class(&RelationSemanticClass::Causal));
    }
}
