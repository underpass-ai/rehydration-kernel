use std::env;
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct PaperMetricRelationship {
    pub(crate) source_node_id: String,
    pub(crate) target_node_id: String,
    pub(crate) relationship_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct PaperUseCaseMetric {
    pub(crate) use_case_id: String,
    pub(crate) variant_id: String,
    pub(crate) variant_label: String,
    pub(crate) relation_variant: String,
    pub(crate) detail_variant: String,
    pub(crate) graph_scale: String,
    pub(crate) requested_token_budget: u32,
    pub(crate) title: String,
    pub(crate) question: String,
    pub(crate) root_node_id: String,
    pub(crate) target_node_id: String,
    pub(crate) bundle_nodes: u32,
    pub(crate) bundle_relationships: u32,
    pub(crate) detailed_nodes: u32,
    pub(crate) rendered_token_count: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) query_latency_ms: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) total_latency_ms: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) graph_load_ms: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) detail_load_ms: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) bundle_assembly_ms: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) detail_batch_size: Option<u32>,
    pub(crate) explanation_roundtrip_fidelity: f64,
    pub(crate) detail_roundtrip_fidelity: f64,
    pub(crate) causal_reconstruction_score: f64,
    pub(crate) rendered_contains_expected_rationale: bool,
    pub(crate) rendered_contains_expected_decision_reference: bool,
    pub(crate) rendered_contains_expected_detail: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) rehydration_point_hit: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) rehydration_node_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) retry_success_hit: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) retry_success_rate: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) retry_target_node_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) dominant_reason_hit: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) suspect_relationship_count: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) full_graph_relationship_count: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) rationale: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) motivation: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) method: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) decision_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) caused_by_node_id: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) suspect_relationships: Vec<PaperMetricRelationship>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) llm_task_success: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) llm_restart_accuracy: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) llm_reason_preserved: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) llm_latency_ms: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) llm_judge_raw: Option<String>,
}

pub(crate) fn ratio(hits: usize, total: usize) -> f64 {
    if total == 0 {
        return 0.0;
    }

    hits as f64 / total as f64
}

pub(crate) fn emit_metric(metric: &PaperUseCaseMetric) -> Result<(), Box<dyn Error + Send + Sync>> {
    let Some(metrics_dir) = env::var_os("REHYDRATION_PAPER_METRICS_DIR") else {
        return Ok(());
    };

    let _guard = metric_write_lock()
        .lock()
        .expect("paper metric write lock should not be poisoned");

    let metrics_dir = PathBuf::from(metrics_dir);
    fs::create_dir_all(&metrics_dir)?;

    let metric_path = metrics_dir.join(format!("{}.json", metric_file_stem(metric)));
    fs::write(&metric_path, serde_json::to_vec_pretty(metric)?)?;

    if let Some(summary_path) = env::var_os("REHYDRATION_PAPER_SUMMARY_PATH") {
        write_summary(&metrics_dir, &PathBuf::from(summary_path))?;
    }

    Ok(())
}

fn write_summary(
    metrics_dir: &Path,
    summary_path: &Path,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    let mut metrics = fs::read_dir(metrics_dir)?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "json")
        })
        .filter(|path| path != summary_path)
        .map(|path| fs::read(&path))
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .filter_map(|bytes| serde_json::from_slice::<PaperUseCaseMetric>(&bytes).ok())
        .collect::<Vec<_>>();

    metrics.sort_by(|left, right| {
        left.use_case_id
            .cmp(&right.use_case_id)
            .then_with(|| left.variant_id.cmp(&right.variant_id))
    });
    fs::write(summary_path, serde_json::to_vec_pretty(&metrics)?)?;

    Ok(())
}

fn metric_write_lock() -> &'static Mutex<()> {
    static METRIC_WRITE_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    METRIC_WRITE_LOCK.get_or_init(|| Mutex::new(()))
}

fn metric_file_stem(metric: &PaperUseCaseMetric) -> String {
    if metric.variant_id == "full_explanatory_with_detail" {
        metric.use_case_id.clone()
    } else {
        format!("{}__{}", metric.use_case_id, metric.variant_id)
    }
}
