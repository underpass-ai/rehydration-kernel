use std::error::Error;
use std::fmt::{Display, Formatter};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DomainError {
    EmptyString {
        context: String,
    },
    EmptyCollection {
        context: String,
    },
    ZeroCount {
        context: String,
    },
    CountExceeds {
        context: String,
        max: usize,
        actual: usize,
    },
    CountBelowMinimum {
        context: String,
        minimum: usize,
        actual: usize,
    },
    DuplicateStepId {
        step_id: String,
    },
    UnsupportedMode {
        value: String,
    },
    UnsupportedTool {
        value: String,
    },
    UnsupportedAnswerPolicy {
        value: String,
    },
    UnsupportedPreparedPayloadSource {
        tool: String,
        source: String,
    },
    UnsupportedFixtureValue {
        context: String,
        value: String,
    },
    DuplicateAllowedTool {
        tool: String,
    },
    ToolOutsideMode {
        mode: String,
        tool: String,
    },
    TargetToolNotAllowed {
        tool: String,
    },
    TrajectoryCaseMismatch {
        field: String,
        expected: String,
        actual: String,
    },
    InvalidActionArguments {
        context: String,
    },
    InvalidActionContract {
        message: String,
    },
}

pub type DomainResult<T> = Result<T, DomainError>;

impl Display for DomainError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyString { context } => write!(f, "{context} must not be empty"),
            Self::EmptyCollection { context } => write!(f, "{context} must not be empty"),
            Self::ZeroCount { context } => write!(f, "{context} must be greater than zero"),
            Self::CountExceeds {
                context,
                max,
                actual,
            } => write!(f, "{context} must be <= {max}; got {actual}"),
            Self::CountBelowMinimum {
                context,
                minimum,
                actual,
            } => write!(f, "{context} must be >= {minimum}; got {actual}"),
            Self::DuplicateStepId { step_id } => {
                write!(f, "duplicate training trajectory step_id `{step_id}`")
            }
            Self::UnsupportedMode { value } => write!(f, "unsupported operator mode `{value}`"),
            Self::UnsupportedTool { value } => write!(f, "unsupported KMP tool `{value}`"),
            Self::UnsupportedAnswerPolicy { value } => {
                write!(f, "unsupported answer policy `{value}`")
            }
            Self::UnsupportedPreparedPayloadSource { tool, source } => write!(
                f,
                "unsupported prepared payload source `{source}` for tool `{tool}`"
            ),
            Self::UnsupportedFixtureValue { context, value } => {
                write!(f, "unsupported fixture value `{value}` for `{context}`")
            }
            Self::DuplicateAllowedTool { tool } => write!(f, "duplicate allowed tool `{tool}`"),
            Self::ToolOutsideMode { mode, tool } => {
                write!(f, "allowed tool `{tool}` is outside mode `{mode}`")
            }
            Self::TargetToolNotAllowed { tool } => {
                write!(f, "target tool `{tool}` is not listed in allowed_tools")
            }
            Self::TrajectoryCaseMismatch {
                field,
                expected,
                actual,
            } => write!(
                f,
                "trajectory field `{field}` must be `{expected}`; got `{actual}`"
            ),
            Self::InvalidActionArguments { context } => {
                write!(f, "{context} must be an object")
            }
            Self::InvalidActionContract { message } => f.write_str(message),
        }
    }
}

impl Error for DomainError {}
