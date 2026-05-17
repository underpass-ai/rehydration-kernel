use crate::{DomainError, DomainResult};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NonEmptyString(String);

impl NonEmptyString {
    pub fn parse(value: impl Into<String>, context: &'static str) -> DomainResult<Self> {
        let value = value.into();
        if value.trim().is_empty() {
            return Err(DomainError::EmptyString {
                context: context.to_string(),
            });
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn into_inner(self) -> String {
        self.0
    }
}

macro_rules! value_object {
    ($name:ident, $context:literal) => {
        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(NonEmptyString);

        impl $name {
            pub fn parse(value: impl Into<String>) -> DomainResult<Self> {
                Ok(Self(NonEmptyString::parse(value, $context)?))
            }

            pub fn as_str(&self) -> &str {
                self.0.as_str()
            }

            pub fn into_inner(self) -> String {
                self.0.into_inner()
            }
        }
    };
}

value_object!(StepId, "step_id");
value_object!(AboutId, "about");
value_object!(TaskFamily, "task_family");
value_object!(MemoryRef, "memory_ref");
value_object!(TrainingRunId, "training_run_id");
value_object!(DatasetId, "dataset_id");
value_object!(SyntheticCaseId, "synthetic_case_id");
value_object!(ModelId, "model_id");
value_object!(ArtifactUri, "artifact_uri");

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ExampleCount(usize);

impl ExampleCount {
    pub fn from_usize(value: usize) -> Self {
        Self(value)
    }

    pub fn as_usize(self) -> usize {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PositiveCount(usize);

impl PositiveCount {
    pub fn parse(value: usize, context: &'static str) -> DomainResult<Self> {
        if value == 0 {
            return Err(DomainError::ZeroCount {
                context: context.to_string(),
            });
        }
        Ok(Self(value))
    }

    pub fn as_usize(self) -> usize {
        self.0
    }

    pub fn as_count(self) -> ExampleCount {
        ExampleCount::from_usize(self.0)
    }
}
