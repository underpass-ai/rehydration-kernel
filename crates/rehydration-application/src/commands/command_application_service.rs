use std::sync::Arc;

use rehydration_domain::ContextEventStore;

use crate::ApplicationError;
use crate::commands::{UpdateContextCommand, UpdateContextOutcome, UpdateContextUseCase};

#[derive(Debug)]
pub struct CommandApplicationService<E> {
    update_context: Arc<UpdateContextUseCase<E>>,
}

impl<E> CommandApplicationService<E>
where
    E: ContextEventStore + Send + Sync,
{
    pub fn new(update_context: Arc<UpdateContextUseCase<E>>) -> Self {
        Self { update_context }
    }

    pub async fn update_context(
        &self,
        command: UpdateContextCommand,
    ) -> Result<UpdateContextOutcome, ApplicationError> {
        let snapshot_id = if command.persist_snapshot {
            Some(format!(
                "snapshot:{}:{}",
                command.root_node_id, command.role
            ))
        } else {
            None
        };

        let mut outcome = self.update_context.execute(command).await?;
        outcome.snapshot_id = snapshot_id;
        Ok(outcome)
    }
}
