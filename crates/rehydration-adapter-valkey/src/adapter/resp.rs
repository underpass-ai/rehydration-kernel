use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::net::TcpStream;

use rehydration_ports::PortError;

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum RespValue {
    SimpleString(String),
    BulkString(Option<String>),
    Integer(i64),
    Error(String),
}

pub(crate) fn encode_set_command(key: &str, payload: &str, ttl_seconds: Option<u64>) -> Vec<u8> {
    let mut arguments = vec!["SET".to_string(), key.to_string(), payload.to_string()];
    if let Some(ttl_seconds) = ttl_seconds {
        arguments.push("EX".to_string());
        arguments.push(ttl_seconds.to_string());
    }

    encode_command(&arguments.iter().map(String::as_str).collect::<Vec<_>>())
}

pub(crate) fn encode_command(arguments: &[&str]) -> Vec<u8> {
    let mut command = Vec::new();
    command.extend_from_slice(format!("*{}\r\n", arguments.len()).as_bytes());
    for argument in arguments {
        command.extend_from_slice(format!("${}\r\n", argument.len()).as_bytes());
        command.extend_from_slice(argument.as_bytes());
        command.extend_from_slice(b"\r\n");
    }

    command
}

pub(crate) async fn read_response(
    reader: &mut BufReader<TcpStream>,
) -> Result<RespValue, PortError> {
    let mut line = String::new();
    reader.read_line(&mut line).await.map_err(|error| {
        PortError::Unavailable(format!("failed to read valkey response: {error}"))
    })?;
    let line = line.trim_end_matches("\r\n");
    let (prefix, remainder) = line.split_at(1);

    match prefix {
        "+" => Ok(RespValue::SimpleString(remainder.to_string())),
        "-" => Ok(RespValue::Error(remainder.to_string())),
        ":" => remainder
            .parse::<i64>()
            .map(RespValue::Integer)
            .map_err(|error| {
                PortError::Unavailable(format!("invalid valkey integer response: {error}"))
            }),
        "$" => {
            let length = remainder.parse::<isize>().map_err(|error| {
                PortError::Unavailable(format!("invalid valkey bulk string response: {error}"))
            })?;
            if length == -1 {
                return Ok(RespValue::BulkString(None));
            }

            let mut buffer = vec![0_u8; length as usize];
            tokio::io::AsyncReadExt::read_exact(reader, &mut buffer)
                .await
                .map_err(|error| {
                    PortError::Unavailable(format!(
                        "failed to read valkey bulk string payload: {error}"
                    ))
                })?;
            let mut crlf = [0_u8; 2];
            tokio::io::AsyncReadExt::read_exact(reader, &mut crlf)
                .await
                .map_err(|error| {
                    PortError::Unavailable(format!(
                        "failed to read valkey bulk string terminator: {error}"
                    ))
                })?;

            let payload = String::from_utf8(buffer).map_err(|error| {
                PortError::Unavailable(format!("invalid valkey bulk string payload: {error}"))
            })?;
            Ok(RespValue::BulkString(Some(payload)))
        }
        other => Err(PortError::Unavailable(format!(
            "unsupported valkey RESP prefix `{other}`"
        ))),
    }
}

pub(crate) fn map_valkey_response(response: RespValue) -> Result<(), PortError> {
    match response {
        RespValue::SimpleString(message) if message == "OK" => Ok(()),
        RespValue::Error(message) => Err(PortError::Unavailable(format!(
            "valkey rejected write: {message}"
        ))),
        other => Err(PortError::Unavailable(format!(
            "unexpected valkey response: {other:?}"
        ))),
    }
}
