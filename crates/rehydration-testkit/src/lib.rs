use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Arc, OnceLock};
use std::time::Duration;

use rehydration_domain::{
    ContextPathNeighborhood, GraphNeighborhoodReader, NodeDetailProjection, NodeDetailReader,
    NodeNeighborhood, PortError, ProcessedEventStore, ProjectionCheckpoint,
    ProjectionCheckpointStore, ProjectionMutation, ProjectionWriter, RehydrationBundle,
    SnapshotSaveOptions, SnapshotStore,
};
use tokio::sync::Mutex;

static TESTCONTAINERS_RUNTIME_INIT: OnceLock<Result<(), String>> = OnceLock::new();

pub fn ensure_testcontainers_runtime() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    TESTCONTAINERS_RUNTIME_INIT
        .get_or_init(|| configure_testcontainers_runtime().map_err(|error| error.to_string()))
        .as_ref()
        .map_err(|message| {
            Box::new(std::io::Error::other(message.clone()))
                as Box<dyn std::error::Error + Send + Sync>
        })?;

    Ok(())
}

fn configure_testcontainers_runtime() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    if docker_is_available() || !command_exists("podman") {
        return Ok(());
    }

    let managed_socket = managed_docker_socket_path()?;

    if path_is_socket(&managed_socket)? {
        return Ok(());
    }

    if let Some(podman_socket) = existing_podman_socket() {
        link_managed_socket(&managed_socket, &podman_socket)?;
        return Ok(());
    }

    if command_exists("systemctl") {
        let _ = Command::new("systemctl")
            .args(["--user", "start", "podman.socket"])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();

        if let Some(podman_socket) = wait_for_podman_socket() {
            link_managed_socket(&managed_socket, &podman_socket)?;
            return Ok(());
        }
    }

    start_podman_service(&managed_socket)
}

fn docker_is_available() -> bool {
    if !command_exists("docker") {
        return false;
    }

    Command::new("docker")
        .arg("info")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
}

fn command_exists(command: &str) -> bool {
    Command::new("sh")
        .args(["-lc", &format!("command -v {command} >/dev/null 2>&1")])
        .status()
        .is_ok_and(|status| status.success())
}

fn managed_docker_socket_path() -> Result<PathBuf, Box<dyn std::error::Error + Send + Sync>> {
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::NotFound, "HOME is not set"))?;
    let socket_path = home.join(".docker/run/docker.sock");
    let parent = socket_path
        .parent()
        .expect("managed socket path should have a parent");
    fs::create_dir_all(parent)?;
    Ok(socket_path)
}

fn existing_podman_socket() -> Option<PathBuf> {
    let runtime_dir = std::env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(format!("/run/user/{}", current_uid())));
    let socket_path = runtime_dir.join("podman/podman.sock");

    path_is_socket(&socket_path)
        .ok()
        .filter(|is_socket| *is_socket)?;
    Some(socket_path)
}

fn wait_for_podman_socket() -> Option<PathBuf> {
    for _ in 0..25 {
        if let Some(socket) = existing_podman_socket() {
            return Some(socket);
        }
        std::thread::sleep(Duration::from_millis(200));
    }

    None
}

fn link_managed_socket(
    managed_socket: &Path,
    target_socket: &Path,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    if path_is_socket(managed_socket)? {
        return Ok(());
    }

    if fs::symlink_metadata(managed_socket).is_ok() {
        fs::remove_file(managed_socket)?;
    }

    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(target_socket, managed_socket)?;
    }

    #[cfg(not(unix))]
    {
        return Err(Box::new(std::io::Error::other(
            "podman fallback symlink is only supported on unix",
        )));
    }

    Ok(())
}

fn start_podman_service(
    managed_socket: &Path,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    if fs::symlink_metadata(managed_socket).is_ok() {
        fs::remove_file(managed_socket)?;
    }

    let log_path = managed_socket.with_extension("log");
    let log_file = fs::File::create(&log_path)?;
    let log_file_err = log_file.try_clone()?;
    let socket_spec = format!("unix://{}", managed_socket.display());

    let child = Command::new("podman")
        .args(["system", "service", "--time=600", socket_spec.as_str()])
        .stdout(Stdio::from(log_file))
        .stderr(Stdio::from(log_file_err))
        .spawn()?;
    std::mem::forget(child);

    for _ in 0..50 {
        if path_is_socket(managed_socket)? {
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(200));
    }

    let log_output = fs::read_to_string(&log_path).unwrap_or_default();
    Err(Box::new(std::io::Error::other(format!(
        "podman socket did not become available at {}{}{}",
        managed_socket.display(),
        if log_output.is_empty() { "" } else { "; log: " },
        log_output
    ))))
}

fn current_uid() -> String {
    std::env::var("UID").unwrap_or_else(|_| {
        Command::new("id")
            .arg("-u")
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .output()
            .ok()
            .and_then(|output| String::from_utf8(output.stdout).ok())
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| "0".to_string())
    })
}

fn path_is_socket(path: &Path) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
    if !path.exists() {
        return Ok(false);
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::FileTypeExt;

        return Ok(fs::metadata(path)?.file_type().is_socket());
    }

    #[cfg(not(unix))]
    {
        Ok(true)
    }
}

#[derive(Debug, Default, Clone)]
pub struct InMemoryGraphNeighborhoodReader {
    neighborhoods: HashMap<String, NodeNeighborhood>,
}

impl InMemoryGraphNeighborhoodReader {
    pub fn with_neighborhood(neighborhood: NodeNeighborhood) -> Self {
        let mut neighborhoods = HashMap::new();
        neighborhoods.insert(neighborhood.root.node_id.clone(), neighborhood);
        Self { neighborhoods }
    }
}

impl GraphNeighborhoodReader for InMemoryGraphNeighborhoodReader {
    async fn load_neighborhood(
        &self,
        root_node_id: &str,
        _depth: u32,
    ) -> Result<Option<NodeNeighborhood>, PortError> {
        Ok(self.neighborhoods.get(root_node_id).cloned())
    }

    async fn load_context_path(
        &self,
        _root_node_id: &str,
        _target_node_id: &str,
        _subtree_depth: u32,
    ) -> Result<Option<ContextPathNeighborhood>, PortError> {
        Ok(None)
    }
}

#[derive(Debug, Default, Clone)]
pub struct InMemoryNodeDetailReader {
    details: HashMap<String, NodeDetailProjection>,
}

impl InMemoryNodeDetailReader {
    pub fn with_details(details: impl IntoIterator<Item = NodeDetailProjection>) -> Self {
        Self {
            details: details
                .into_iter()
                .map(|detail| (detail.node_id.clone(), detail))
                .collect(),
        }
    }
}

impl NodeDetailReader for InMemoryNodeDetailReader {
    async fn load_node_detail(
        &self,
        node_id: &str,
    ) -> Result<Option<NodeDetailProjection>, PortError> {
        Ok(self.details.get(node_id).cloned())
    }
}

#[derive(Debug, Default, Clone)]
pub struct InMemoryProjectionWriter {
    mutations: Arc<Mutex<Vec<ProjectionMutation>>>,
}

impl InMemoryProjectionWriter {
    pub async fn mutations(&self) -> Vec<ProjectionMutation> {
        self.mutations.lock().await.clone()
    }
}

impl ProjectionWriter for InMemoryProjectionWriter {
    async fn apply_mutations(&self, mutations: Vec<ProjectionMutation>) -> Result<(), PortError> {
        self.mutations.lock().await.extend(mutations);
        Ok(())
    }
}

#[derive(Debug, Default, Clone)]
pub struct InMemoryProcessedEventStore {
    processed: Arc<Mutex<HashSet<(String, String)>>>,
}

impl InMemoryProcessedEventStore {
    pub async fn processed(&self) -> HashSet<(String, String)> {
        self.processed.lock().await.clone()
    }
}

impl ProcessedEventStore for InMemoryProcessedEventStore {
    async fn has_processed(&self, consumer_name: &str, event_id: &str) -> Result<bool, PortError> {
        Ok(self
            .processed
            .lock()
            .await
            .contains(&(consumer_name.to_string(), event_id.to_string())))
    }

    async fn record_processed(&self, consumer_name: &str, event_id: &str) -> Result<(), PortError> {
        self.processed
            .lock()
            .await
            .insert((consumer_name.to_string(), event_id.to_string()));
        Ok(())
    }
}

#[derive(Debug, Default, Clone)]
pub struct InMemoryProjectionCheckpointStore {
    checkpoints: Arc<Mutex<HashMap<(String, String), ProjectionCheckpoint>>>,
}

impl InMemoryProjectionCheckpointStore {
    pub async fn checkpoint(
        &self,
        consumer_name: &str,
        stream_name: &str,
    ) -> Option<ProjectionCheckpoint> {
        self.checkpoints
            .lock()
            .await
            .get(&(consumer_name.to_string(), stream_name.to_string()))
            .cloned()
    }
}

impl ProjectionCheckpointStore for InMemoryProjectionCheckpointStore {
    async fn load_checkpoint(
        &self,
        consumer_name: &str,
        stream_name: &str,
    ) -> Result<Option<ProjectionCheckpoint>, PortError> {
        Ok(self
            .checkpoints
            .lock()
            .await
            .get(&(consumer_name.to_string(), stream_name.to_string()))
            .cloned())
    }

    async fn save_checkpoint(&self, checkpoint: ProjectionCheckpoint) -> Result<(), PortError> {
        let key = (
            checkpoint.consumer_name.clone(),
            checkpoint.stream_name.clone(),
        );
        self.checkpoints.lock().await.insert(key, checkpoint);
        Ok(())
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub struct NoopSnapshotStore;

impl SnapshotStore for NoopSnapshotStore {
    async fn save_bundle_with_options(
        &self,
        _bundle: &RehydrationBundle,
        _options: SnapshotSaveOptions,
    ) -> Result<(), PortError> {
        Ok(())
    }
}

#[cfg(test)]
fn seed_bundle(case_id: rehydration_domain::CaseId, role: &str) -> RehydrationBundle {
    let role = rehydration_domain::Role::new(role).expect("role must be valid");
    RehydrationBundle::new(
        case_id.clone(),
        role.clone(),
        rehydration_domain::BundleNode::new(
            case_id.as_str(),
            "capability",
            format!("Node {}", case_id.as_str()),
            format!(
                "bundle for node {} role {}",
                case_id.as_str(),
                role.as_str()
            ),
            "ACTIVE",
            vec!["projection-node".to_string()],
            std::collections::BTreeMap::new(),
        ),
        Vec::new(),
        Vec::new(),
        vec![rehydration_domain::BundleNodeDetail::new(
            case_id.as_str(),
            format!(
                "bundle for node {} role {}",
                case_id.as_str(),
                role.as_str()
            ),
            "pending",
            1,
        )],
        rehydration_domain::BundleMetadata::initial("0.1.0"),
    )
    .expect("seed bundle should be valid")
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use rehydration_domain::{
        CaseId, GraphNeighborhoodReader, NodeDetailProjection, NodeDetailReader, NodeNeighborhood,
        NodeProjection, ProcessedEventStore, ProjectionCheckpoint, ProjectionCheckpointStore,
        ProjectionMutation, ProjectionWriter, SnapshotStore,
    };

    use super::{
        InMemoryGraphNeighborhoodReader, InMemoryNodeDetailReader, InMemoryProcessedEventStore,
        InMemoryProjectionCheckpointStore, InMemoryProjectionWriter, NoopSnapshotStore,
        seed_bundle,
    };

    #[tokio::test]
    async fn in_memory_graph_reader_returns_seeded_neighborhood() {
        let reader = InMemoryGraphNeighborhoodReader::with_neighborhood(NodeNeighborhood {
            root: NodeProjection {
                node_id: "node-123".to_string(),
                node_kind: "capability".to_string(),
                title: "Projection".to_string(),
                summary: String::new(),
                status: "ACTIVE".to_string(),
                labels: vec!["projection".to_string()],
                properties: BTreeMap::new(),
            },
            neighbors: Vec::new(),
            relations: Vec::new(),
        });

        let loaded = reader
            .load_neighborhood("node-123", 1)
            .await
            .expect("load should succeed");

        assert!(loaded.is_some());
    }

    #[tokio::test]
    async fn in_memory_node_detail_reader_returns_seeded_detail() {
        let reader = InMemoryNodeDetailReader::with_details([NodeDetailProjection {
            node_id: "node-123".to_string(),
            detail: "Expanded detail".to_string(),
            content_hash: "hash-1".to_string(),
            revision: 1,
        }]);

        let loaded = reader
            .load_node_detail("node-123")
            .await
            .expect("load should succeed");

        assert!(loaded.is_some());
    }

    #[tokio::test]
    async fn in_memory_projection_writer_records_mutations() {
        let writer = InMemoryProjectionWriter::default();
        writer
            .apply_mutations(vec![
                ProjectionMutation::UpsertNode(NodeProjection {
                    node_id: "node-123".to_string(),
                    node_kind: "task".to_string(),
                    title: "Task 123".to_string(),
                    summary: "Projection updated".to_string(),
                    status: "ACTIVE".to_string(),
                    labels: vec!["work-item".to_string()],
                    properties: BTreeMap::new(),
                }),
                ProjectionMutation::UpsertNodeDetail(NodeDetailProjection {
                    node_id: "node-123".to_string(),
                    detail: "expanded node detail".to_string(),
                    content_hash: "hash-1".to_string(),
                    revision: 1,
                }),
            ])
            .await
            .expect("write should succeed");

        assert_eq!(writer.mutations().await.len(), 2);
    }

    #[tokio::test]
    async fn in_memory_processed_event_store_tracks_deduplication() {
        let store = InMemoryProcessedEventStore::default();
        assert!(
            !store
                .has_processed("context-projection", "evt-1")
                .await
                .expect("lookup should succeed")
        );

        store
            .record_processed("context-projection", "evt-1")
            .await
            .expect("record should succeed");

        assert!(
            store
                .has_processed("context-projection", "evt-1")
                .await
                .expect("lookup should succeed")
        );
        assert_eq!(store.processed().await.len(), 1);
    }

    #[tokio::test]
    async fn in_memory_checkpoint_store_persists_latest_checkpoint() {
        let store = InMemoryProjectionCheckpointStore::default();
        store
            .save_checkpoint(ProjectionCheckpoint {
                consumer_name: "context-projection".to_string(),
                stream_name: "rehydration.events".to_string(),
                last_subject: "graph.node.materialized".to_string(),
                last_event_id: "evt-1".to_string(),
                last_correlation_id: "corr-1".to_string(),
                last_occurred_at: "2026-03-07T00:00:00Z".to_string(),
                processed_events: 1,
                updated_at: std::time::SystemTime::UNIX_EPOCH,
            })
            .await
            .expect("save should succeed");

        let checkpoint = store
            .checkpoint("context-projection", "rehydration.events")
            .await
            .expect("checkpoint should exist");
        assert_eq!(checkpoint.last_event_id, "evt-1");
    }

    #[tokio::test]
    async fn noop_snapshot_store_accepts_bundle() {
        let bundle = seed_bundle(
            CaseId::new("case-123").expect("case id is valid"),
            "developer",
        );

        NoopSnapshotStore
            .save_bundle(&bundle)
            .await
            .expect("noop snapshot store should accept bundles");
    }
}
