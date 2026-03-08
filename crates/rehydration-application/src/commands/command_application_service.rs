#[derive(Debug)]
pub struct CommandApplicationService {
    update_context: std::sync::Arc<crate::commands::UpdateContextUseCase>,
}

impl CommandApplicationService {
    pub fn new(update_context: std::sync::Arc<crate::commands::UpdateContextUseCase>) -> Self {
        Self { update_context }
    }

    pub fn update_context(
        &self,
        command: crate::commands::UpdateContextCommand,
    ) -> Result<crate::commands::UpdateContextOutcome, crate::ApplicationError> {
        let snapshot_id = if command.persist_snapshot {
            Some(format!("snapshot:{}:{}", command.case_id, command.role))
        } else {
            None
        };

        let mut outcome = self.update_context.execute(command)?;
        outcome.snapshot_id = snapshot_id;
        Ok(outcome)
    }
}
