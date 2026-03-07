use std::error::Error;
use std::fmt;

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
pub struct RehydrationBundle {
    case_id: CaseId,
    role: Role,
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
        Self {
            case_id,
            role,
            sections,
            metadata,
        }
    }

    pub fn empty(case_id: CaseId, role: Role, generator_version: &str) -> Self {
        Self::new(
            case_id,
            role,
            Vec::new(),
            BundleMetadata::initial(generator_version),
        )
    }

    pub fn case_id(&self) -> &CaseId {
        &self.case_id
    }

    pub fn role(&self) -> &Role {
        &self.role
    }

    pub fn sections(&self) -> &[String] {
        &self.sections
    }

    pub fn metadata(&self) -> &BundleMetadata {
        &self.metadata
    }
}

#[cfg(test)]
mod tests {
    use super::{CaseId, DomainError, RehydrationBundle, Role};

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
        assert!(bundle.sections().is_empty());
    }
}
