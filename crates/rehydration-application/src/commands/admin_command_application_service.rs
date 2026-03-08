#[derive(Debug, Default, Clone, Copy)]
pub struct AdminCommandApplicationService;

impl AdminCommandApplicationService {
    pub fn replay_projection(
        &self,
        command: crate::commands::ReplayProjectionCommand,
    ) -> Result<crate::commands::ReplayProjectionOutcome, crate::ApplicationError> {
        let consumer_name = trim_to_option(&command.consumer_name).ok_or_else(|| {
            crate::ApplicationError::Validation("consumer_name cannot be empty".to_string())
        })?;
        let stream_name = trim_to_option(&command.stream_name).ok_or_else(|| {
            crate::ApplicationError::Validation("stream_name cannot be empty".to_string())
        })?;

        Ok(crate::commands::ReplayProjectionOutcome {
            replay_id: format!("replay:{consumer_name}:{stream_name}"),
            consumer_name,
            replay_mode: command.replay_mode,
            accepted_events: command.max_events,
            requested_at: std::time::SystemTime::now(),
        })
    }
}

fn trim_to_option(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}
