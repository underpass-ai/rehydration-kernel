use std::time::SystemTime;

use crate::CaseId;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaseHeader {
    case_id: CaseId,
    title: String,
    summary: String,
    status: String,
    created_at: SystemTime,
    created_by: String,
}

impl CaseHeader {
    pub fn new(
        case_id: CaseId,
        title: impl Into<String>,
        summary: impl Into<String>,
        status: impl Into<String>,
        created_at: SystemTime,
        created_by: impl Into<String>,
    ) -> Self {
        Self {
            case_id,
            title: title.into(),
            summary: summary.into(),
            status: status.into(),
            created_at,
            created_by: created_by.into(),
        }
    }

    pub fn case_id(&self) -> &CaseId {
        &self.case_id
    }

    pub fn title(&self) -> &str {
        &self.title
    }

    pub fn summary(&self) -> &str {
        &self.summary
    }

    pub fn status(&self) -> &str {
        &self.status
    }

    pub fn created_at(&self) -> SystemTime {
        self.created_at
    }

    pub fn created_by(&self) -> &str {
        &self.created_by
    }
}
