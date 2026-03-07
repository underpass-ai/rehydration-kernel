use rehydration_application::{ApplicationError, RehydrateSessionUseCase, RehydrationApplication};
use rehydration_config::AppConfig;
use rehydration_domain::RehydrationBundle;
use rehydration_ports::{ProjectionReader, SnapshotStore};

#[derive(Debug)]
pub struct GrpcServer<R, S> {
    bind_addr: String,
    use_case: RehydrateSessionUseCase<R, S>,
    capability_name: &'static str,
}

impl<R, S> GrpcServer<R, S>
where
    R: ProjectionReader,
    S: SnapshotStore,
{
    pub fn new(config: AppConfig, projection_reader: R, snapshot_store: S) -> Self {
        Self {
            bind_addr: config.grpc_bind,
            use_case: RehydrateSessionUseCase::new(
                projection_reader,
                snapshot_store,
                env!("CARGO_PKG_VERSION"),
            ),
            capability_name: RehydrationApplication::capability_name(),
        }
    }

    pub fn describe(&self) -> String {
        format!(
            "grpc transport placeholder for {} on {}",
            self.capability_name, self.bind_addr
        )
    }

    pub fn warmup_bundle(&self) -> Result<RehydrationBundle, ApplicationError> {
        self.use_case.execute("bootstrap-case", "system")
    }
}

#[cfg(test)]
mod tests {
    use rehydration_domain::{CaseId, RehydrationBundle, Role};
    use rehydration_ports::{PortError, ProjectionReader, SnapshotStore};

    use super::GrpcServer;

    struct EmptyProjectionReader;

    impl ProjectionReader for EmptyProjectionReader {
        fn load_bundle(
            &self,
            _case_id: &CaseId,
            _role: &Role,
        ) -> Result<Option<RehydrationBundle>, PortError> {
            Ok(None)
        }
    }

    struct NoopSnapshotStore;

    impl SnapshotStore for NoopSnapshotStore {
        fn save_bundle(&self, _bundle: &RehydrationBundle) -> Result<(), PortError> {
            Ok(())
        }
    }

    #[test]
    fn describe_mentions_bind_address() {
        let config = rehydration_config::AppConfig {
            service_name: "rehydration-kernel".to_string(),
            grpc_bind: "127.0.0.1:50054".to_string(),
            admin_bind: "127.0.0.1:8080".to_string(),
            graph_uri: "neo4j://localhost:7687".to_string(),
            snapshot_uri: "redis://localhost:6379".to_string(),
            events_subject_prefix: "rehydration".to_string(),
        };
        let server = GrpcServer::new(config, EmptyProjectionReader, NoopSnapshotStore);

        assert!(server.describe().contains("127.0.0.1:50054"));
    }
}
