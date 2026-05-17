use std::collections::{BTreeMap, VecDeque};
use std::error::Error;
use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::time::Instant;

use serde_json::Value;
use underpass_operator_replay_domain::{
    ReplayActionDecision, ReplayCounters, ReplayPrediction, ReplayRunReport, ReplaySummaryMetadata,
    ReplayToolCallPlan, ReplayTrajectory, build_replay_summary, decide_replay_action,
    tool_error_row, tool_success_row,
};

#[derive(Debug, Clone)]
pub struct ReplayMcpPredictionsRequest {
    pub generated_at_unix_seconds: u64,
    pub endpoint: String,
    pub trajectories_label: String,
    pub predictions_label: String,
    pub output_label: String,
    pub starting_request_id: u64,
    pub trajectories: Vec<ReplayTrajectory>,
    pub predictions: BTreeMap<String, VecDeque<ReplayPrediction>>,
}

#[derive(Debug, Clone)]
pub struct ReplayToolCallRequest {
    pub request_id: u64,
    pub name: String,
    pub arguments: Value,
}

#[derive(Debug, Clone)]
pub struct ReplayToolCallResponse {
    pub structured_content: Value,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReplayApplicationError {
    message: String,
}

impl ReplayApplicationError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }

    pub fn message(&self) -> &str {
        &self.message
    }
}

impl fmt::Display for ReplayApplicationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl Error for ReplayApplicationError {}

pub trait ReplayToolCaller {
    fn call_tool<'a>(
        &'a self,
        request: ReplayToolCallRequest,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<ReplayToolCallResponse, ReplayApplicationError>> + Send + 'a,
        >,
    >;
}

pub struct ReplayProgress<'a> {
    pub processed: usize,
    pub total: usize,
    pub row: &'a underpass_operator_replay_domain::ReplayRow,
    pub elapsed_ms: u128,
}

pub trait ReplayProgressObserver {
    fn observe(&self, progress: ReplayProgress<'_>);
}

#[derive(Debug, Clone, Copy, Default)]
pub struct NoopReplayProgressObserver;

impl ReplayProgressObserver for NoopReplayProgressObserver {
    fn observe(&self, _progress: ReplayProgress<'_>) {}
}

pub struct ReplayMcpPredictionsUseCase<C, P = NoopReplayProgressObserver> {
    caller: C,
    progress_observer: P,
}

impl<C> ReplayMcpPredictionsUseCase<C, NoopReplayProgressObserver>
where
    C: ReplayToolCaller,
{
    pub fn new(caller: C) -> Self {
        Self {
            caller,
            progress_observer: NoopReplayProgressObserver,
        }
    }
}

impl<C, P> ReplayMcpPredictionsUseCase<C, P>
where
    C: ReplayToolCaller,
    P: ReplayProgressObserver,
{
    pub fn new_with_progress(caller: C, progress_observer: P) -> Self {
        Self {
            caller,
            progress_observer,
        }
    }

    pub async fn execute(
        &self,
        mut request: ReplayMcpPredictionsRequest,
    ) -> Result<ReplayRunReport, ReplayApplicationError> {
        let started = Instant::now();
        let mut request_id = request.starting_request_id;
        let mut rows = Vec::new();
        let mut counters = ReplayCounters::default();
        let total = request.trajectories.len();

        for trajectory in &request.trajectories {
            match decide_replay_action(
                trajectory,
                take_prediction(&mut request.predictions, &trajectory.step_id).as_ref(),
            ) {
                ReplayActionDecision::MissingPrediction(row) => {
                    counters.record_missing_prediction();
                    rows.push(row);
                    self.observe_progress(&rows, started.elapsed().as_millis(), total);
                }
                ReplayActionDecision::InvalidPrediction {
                    row,
                    unbounded_tool_call,
                    contract_violation_phase,
                } => {
                    counters.record_invalid_prediction(
                        unbounded_tool_call,
                        contract_violation_phase.as_deref(),
                    );
                    rows.push(row);
                    self.observe_progress(&rows, started.elapsed().as_millis(), total);
                }
                ReplayActionDecision::Stop(row) => {
                    counters.record_stop(&row);
                    rows.push(row);
                    self.observe_progress(&rows, started.elapsed().as_millis(), total);
                }
                ReplayActionDecision::ToolCall(plan) => {
                    counters.record_tool_call_started();
                    let row = self.execute_tool_call(trajectory, &plan, request_id).await;
                    request_id = request_id.saturating_add(1);
                    counters.record_tool_call_result(&row);
                    rows.push(row);
                    self.observe_progress(&rows, started.elapsed().as_millis(), total);
                }
            }
        }

        let summary = build_replay_summary(
            ReplaySummaryMetadata {
                generated_at_unix_seconds: request.generated_at_unix_seconds,
                endpoint: request.endpoint,
                trajectories: request.trajectories_label,
                predictions: request.predictions_label,
                output: request.output_label,
                elapsed_ms: started.elapsed().as_millis(),
            },
            request.trajectories.len(),
            counters,
            &rows,
        );

        Ok(ReplayRunReport { summary, rows })
    }

    async fn execute_tool_call(
        &self,
        trajectory: &ReplayTrajectory,
        plan: &ReplayToolCallPlan,
        request_id: u64,
    ) -> underpass_operator_replay_domain::ReplayRow {
        let started = Instant::now();
        match self
            .caller
            .call_tool(ReplayToolCallRequest {
                request_id,
                name: plan.tool.clone(),
                arguments: plan.arguments.clone(),
            })
            .await
        {
            Ok(response) => tool_success_row(
                trajectory,
                plan,
                started.elapsed().as_millis(),
                &response.structured_content,
            ),
            Err(error) => tool_error_row(trajectory, plan, error.to_string()),
        }
    }

    fn observe_progress(
        &self,
        rows: &[underpass_operator_replay_domain::ReplayRow],
        elapsed_ms: u128,
        total: usize,
    ) {
        let Some(row) = rows.last() else {
            return;
        };
        self.progress_observer.observe(ReplayProgress {
            processed: rows.len(),
            total,
            row,
            elapsed_ms,
        });
    }
}

fn take_prediction(
    predictions: &mut BTreeMap<String, VecDeque<ReplayPrediction>>,
    step_id: &str,
) -> Option<ReplayPrediction> {
    predictions.get_mut(step_id).and_then(VecDeque::pop_front)
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, VecDeque};

    use serde_json::json;

    use super::*;

    struct StaticCaller;

    impl ReplayToolCaller for StaticCaller {
        fn call_tool<'a>(
            &'a self,
            _request: ReplayToolCallRequest,
        ) -> Pin<
            Box<
                dyn Future<Output = Result<ReplayToolCallResponse, ReplayApplicationError>>
                    + Send
                    + 'a,
            >,
        > {
            Box::pin(async {
                Ok(ReplayToolCallResponse {
                    structured_content: json!({
                        "proof": {
                            "refs": ["incident:run:node:a"]
                        }
                    }),
                })
            })
        }
    }

    #[tokio::test]
    async fn replays_tool_call_through_port_and_builds_summary() {
        let request = ReplayMcpPredictionsRequest {
            generated_at_unix_seconds: 1,
            endpoint: "test".to_string(),
            trajectories_label: "trajectories.jsonl".to_string(),
            predictions_label: "predictions.jsonl".to_string(),
            output_label: "out".to_string(),
            starting_request_id: 1,
            trajectories: vec![ReplayTrajectory {
                step_id: "s1".to_string(),
                about: "about:1".to_string(),
                mode: "read".to_string(),
                task_family: "test".to_string(),
                allowed_tools: vec!["kernel_inspect".to_string()],
                observed_outcome: Some(json!({
                    "observed_refs": ["incident:run:node:a"]
                })),
            }],
            predictions: BTreeMap::from([(
                "s1".to_string(),
                VecDeque::from([ReplayPrediction {
                    action: json!({
                        "type": "tool_call",
                        "tool": "kernel_inspect",
                        "arguments": {
                            "ref": "incident:run:node:a",
                            "include": {
                                "details": true,
                                "incoming": true,
                                "outgoing": true,
                                "raw": false
                            }
                        }
                    }),
                }]),
            )]),
        };

        let report = ReplayMcpPredictionsUseCase::new(StaticCaller)
            .execute(request)
            .await
            .expect("report");

        assert_eq!(report.summary.selected, 1);
        assert_eq!(report.summary.successful_tool_calls, 1);
        assert_eq!(report.summary.missing_expected_ref_rows, 0);
    }
}

#[cfg(test)]
mod dependency_tests {
    use std::fs;
    use std::path::Path;

    #[test]
    fn crate_has_no_rehydration_dependencies() {
        let manifest = Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
        let contents = fs::read_to_string(manifest).expect("manifest must be readable");

        assert!(
            !contents.contains("rehydration-"),
            "underpass-operator-replay-application must stay independent from kernel crates"
        );
    }
}
