use rehydration_domain::{CaseId, Role};

use crate::ApplicationError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpdateContextChange {
    pub operation: String,
    pub entity_kind: String,
    pub entity_id: String,
    pub payload_json: String,
    pub reason: String,
    pub scopes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpdateContextCommand {
    pub root_node_id: String,
    pub role: String,
    pub work_item_id: String,
    pub changes: Vec<UpdateContextChange>,
    pub expected_revision: Option<u64>,
    pub expected_content_hash: Option<String>,
    pub idempotency_key: Option<String>,
    pub requested_by: Option<String>,
    pub persist_snapshot: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AcceptedVersion {
    pub revision: u64,
    pub content_hash: String,
    pub generator_version: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpdateContextOutcome {
    pub accepted_version: AcceptedVersion,
    pub warnings: Vec<String>,
    pub snapshot_persisted: bool,
    pub snapshot_id: Option<String>,
}

#[derive(Debug)]
pub struct UpdateContextUseCase {
    generator_version: &'static str,
}

impl UpdateContextUseCase {
    pub fn new(generator_version: &'static str) -> Self {
        Self { generator_version }
    }

    pub fn execute(
        &self,
        command: UpdateContextCommand,
    ) -> Result<UpdateContextOutcome, ApplicationError> {
        let case_id = CaseId::new(command.root_node_id)?;
        let role = Role::new(command.role)?;

        let revision = command.expected_revision.unwrap_or(0) + 1;
        let content_hash = format!(
            "{}:{}:{}:{}",
            case_id.as_str(),
            role.as_str(),
            command.work_item_id,
            command.changes.len()
        );

        let mut warnings = Vec::new();
        if command.changes.is_empty() {
            warnings.push("no changes supplied; update was accepted as a no-op".to_string());
        }
        if command.expected_content_hash.is_none() {
            warnings.push(
                "expected_content_hash missing; optimistic verification is partial".to_string(),
            );
        }
        if command.idempotency_key.is_none() {
            warnings.push(
                "idempotency_key missing; duplicate suppression is delegated upstream".to_string(),
            );
        }

        Ok(UpdateContextOutcome {
            accepted_version: AcceptedVersion {
                revision,
                content_hash,
                generator_version: self.generator_version.to_string(),
            },
            warnings,
            snapshot_persisted: command.persist_snapshot,
            snapshot_id: None,
        })
    }
}
