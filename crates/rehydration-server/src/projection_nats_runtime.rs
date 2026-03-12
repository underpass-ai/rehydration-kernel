use std::error::Error;

use rehydration_adapter_nats::NatsProjectionRuntime;
use rehydration_adapter_valkey::{ValkeyProcessedEventStore, ValkeyProjectionCheckpointStore};
use rehydration_application::{ProjectionApplicationService, RoutingProjectionWriter};
use rehydration_config::ProjectionRuntimeConfig;
use rehydration_domain::ProjectionWriter;

pub enum ProjectionRuntime<H> {
    Enabled(Box<NatsProjectionRuntime<H>>),
    Disabled,
}

impl<H> ProjectionRuntime<H>
where
    H: rehydration_application::ProjectionEventHandler + Send + Sync + 'static,
{
    pub fn describe(&self) -> String {
        match self {
            Self::Enabled(runtime) => runtime.describe(),
            Self::Disabled => "nats projection runtime disabled".to_string(),
        }
    }

    pub async fn run(self) -> Result<(), Box<dyn Error + Send + Sync>> {
        match self {
            Self::Enabled(runtime) => runtime
                .run()
                .await
                .map_err(|error| Box::new(error) as Box<dyn Error + Send + Sync>),
            Self::Disabled => Ok(()),
        }
    }
}

pub async fn connect_projection_runtime<G, D>(
    config: &ProjectionRuntimeConfig,
    subject_prefix: &str,
    graph_writer: G,
    detail_writer: D,
) -> Result<
    ProjectionRuntime<
        ProjectionApplicationService<
            RoutingProjectionWriter<G, D>,
            ValkeyProcessedEventStore,
            ValkeyProjectionCheckpointStore,
        >,
    >,
    Box<dyn Error + Send + Sync>,
>
where
    G: ProjectionWriter + Send + Sync + 'static,
    D: ProjectionWriter + Send + Sync + 'static,
{
    if !config.enabled {
        return Ok(ProjectionRuntime::Disabled);
    }

    let processed_event_store = ValkeyProcessedEventStore::new(config.runtime_state_uri.clone())?;
    let checkpoint_store = ValkeyProjectionCheckpointStore::new(config.runtime_state_uri.clone())?;
    let handler = ProjectionApplicationService::new(
        RoutingProjectionWriter::new(graph_writer, detail_writer),
        processed_event_store,
        checkpoint_store,
    );

    NatsProjectionRuntime::connect(&config.nats_url, subject_prefix, handler)
        .await
        .map(Box::new)
        .map(ProjectionRuntime::Enabled)
        .map_err(|error| Box::new(error) as Box<dyn Error + Send + Sync>)
}

#[cfg(test)]
mod tests {
    use rehydration_config::ProjectionRuntimeConfig;
    use rehydration_domain::{PortError, ProjectionMutation, ProjectionWriter};

    use super::connect_projection_runtime;

    #[derive(Debug, Clone, Copy)]
    struct NoopProjectionWriter;

    impl ProjectionWriter for NoopProjectionWriter {
        async fn apply_mutations(
            &self,
            _mutations: Vec<ProjectionMutation>,
        ) -> Result<(), PortError> {
            Ok(())
        }
    }

    #[tokio::test]
    async fn disabled_projection_nats_returns_disabled_runtime() {
        let runtime = connect_projection_runtime(
            &ProjectionRuntimeConfig {
                nats_url: "nats://127.0.0.1:4222".to_string(),
                enabled: false,
                runtime_state_uri: "redis://127.0.0.1:6379".to_string(),
            },
            "rehydration",
            NoopProjectionWriter,
            NoopProjectionWriter,
        )
        .await
        .expect("disabled projection runtime should still allow startup");

        assert_eq!(runtime.describe(), "nats projection runtime disabled");
    }

    #[tokio::test]
    async fn enabled_projection_runtime_surfaces_connection_errors() {
        let error = match connect_projection_runtime(
            &ProjectionRuntimeConfig {
                nats_url: "nats://127.0.0.1:1".to_string(),
                enabled: true,
                runtime_state_uri: "redis://127.0.0.1:6379".to_string(),
            },
            "rehydration",
            NoopProjectionWriter,
            NoopProjectionWriter,
        )
        .await
        {
            Ok(_) => panic!("invalid nats endpoint should fail when projection runtime is enabled"),
            Err(error) => error,
        };

        assert!(
            error.to_string().contains("connection"),
            "unexpected error: {error}"
        );
    }
}
