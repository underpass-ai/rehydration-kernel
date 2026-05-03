use std::io::Write;
use std::process::{Command, Stdio};

use serde_json::Value;

const TLS_ENV_VARS: &[&str] = &[
    "REHYDRATION_KERNEL_GRPC_ENDPOINT",
    "REHYDRATION_KERNEL_GRPC_TLS_MODE",
    "REHYDRATION_KERNEL_GRPC_TLS_CA_PATH",
    "REHYDRATION_KERNEL_GRPC_TLS_CERT_PATH",
    "REHYDRATION_KERNEL_GRPC_TLS_KEY_PATH",
    "REHYDRATION_KERNEL_GRPC_TLS_DOMAIN_NAME",
];

#[test]
fn stdio_binary_serves_fixture_jsonrpc_until_stdin_eof() {
    let output = run_binary(
        &[],
        "\n\
         {\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\",\"params\":{}}\n\
         {\"jsonrpc\":\"2.0\",\"method\":\"notifications/initialized\",\"params\":{}}\n\
         {\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"tools/list\",\"params\":{}}\n",
    );

    assert!(output.status.success());
    let stderr = String::from_utf8(output.stderr).expect("stderr should be UTF-8");
    assert!(stderr.contains("using fixture backend"));

    let stdout = String::from_utf8(output.stdout).expect("stdout should be UTF-8");
    let responses = stdout
        .lines()
        .map(|line| serde_json::from_str::<Value>(line).expect("stdout line should be JSON"))
        .collect::<Vec<_>>();

    assert_eq!(responses.len(), 2);
    assert_eq!(responses[0]["id"], 1);
    assert_eq!(responses[0]["result"]["metadata"]["backend"], "fixture");
    assert_eq!(responses[1]["id"], 2);
    assert_eq!(
        responses[1]["result"]["tools"][0]["name"],
        Value::String("kernel_ingest".to_string())
    );
}

#[test]
fn stdio_binary_reports_live_grpc_backend_without_tls() {
    let output = run_binary(
        &[("REHYDRATION_KERNEL_GRPC_ENDPOINT", "http://127.0.0.1:1")],
        "",
    );

    assert!(output.status.success());
    let stderr = String::from_utf8(output.stderr).expect("stderr should be UTF-8");
    assert!(stderr.contains("using live gRPC backend"));
    assert!(stderr.contains("REHYDRATION_KERNEL_GRPC_TLS_MODE=disabled"));
    assert!(!stderr.contains("TLS envs:"));
}

#[test]
fn stdio_binary_reports_live_grpc_backend_with_tls_envs() {
    let output = run_binary(
        &[(
            "REHYDRATION_KERNEL_GRPC_ENDPOINT",
            "https://rehydration-kernel.example.test",
        )],
        "",
    );

    assert!(output.status.success());
    let stderr = String::from_utf8(output.stderr).expect("stderr should be UTF-8");
    assert!(stderr.contains("using live gRPC backend"));
    assert!(stderr.contains("REHYDRATION_KERNEL_GRPC_TLS_MODE=server"));
    assert!(stderr.contains("TLS envs:"));
}

fn run_binary(envs: &[(&str, &str)], stdin: &str) -> std::process::Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_rehydration-mcp"));
    command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    for name in TLS_ENV_VARS {
        command.env_remove(name);
    }
    for (name, value) in envs {
        command.env(name, value);
    }

    let mut child = command.spawn().expect("stdio MCP binary should spawn");
    child
        .stdin
        .as_mut()
        .expect("stdin should be piped")
        .write_all(stdin.as_bytes())
        .expect("stdin should be written");
    drop(child.stdin.take());

    child
        .wait_with_output()
        .expect("stdio MCP binary should exit after stdin EOF")
}
