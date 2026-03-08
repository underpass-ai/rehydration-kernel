use tokio::io::AsyncWriteExt;
use tokio::io::BufReader;
use tokio::net::TcpStream;

use rehydration_ports::PortError;

use crate::adapter::endpoint::ValkeyEndpoint;
use crate::adapter::resp::{
    RespValue, encode_command, encode_set_command, map_valkey_response, read_response,
};

pub(crate) async fn execute_set_command(
    endpoint: &ValkeyEndpoint,
    key: &str,
    payload: &str,
) -> Result<(), PortError> {
    let mut stream = TcpStream::connect(endpoint.address())
        .await
        .map_err(|error| {
            PortError::Unavailable(format!(
                "unable to connect to valkey {}: {error}",
                endpoint.raw_uri
            ))
        })?;

    let frame = encode_set_command(key, payload, endpoint.ttl_seconds);
    stream.write_all(&frame).await.map_err(|error| {
        PortError::Unavailable(format!("failed to write valkey payload: {error}"))
    })?;
    stream.flush().await.map_err(|error| {
        PortError::Unavailable(format!("failed to flush valkey payload: {error}"))
    })?;

    let mut reader = BufReader::new(stream);
    map_valkey_response(read_response(&mut reader).await?)
}

pub(crate) async fn execute_get_command(
    endpoint: &ValkeyEndpoint,
    key: &str,
) -> Result<Option<String>, PortError> {
    let mut stream = TcpStream::connect(endpoint.address())
        .await
        .map_err(|error| {
            PortError::Unavailable(format!(
                "unable to connect to valkey {}: {error}",
                endpoint.raw_uri
            ))
        })?;

    let frame = encode_command(&["GET", key]);
    stream.write_all(&frame).await.map_err(|error| {
        PortError::Unavailable(format!("failed to write valkey command: {error}"))
    })?;
    stream.flush().await.map_err(|error| {
        PortError::Unavailable(format!("failed to flush valkey command: {error}"))
    })?;

    let mut reader = BufReader::new(stream);
    match read_response(&mut reader).await? {
        RespValue::BulkString(payload) => Ok(payload),
        RespValue::Error(message) => Err(PortError::Unavailable(format!(
            "valkey rejected read: {message}"
        ))),
        other => Err(PortError::Unavailable(format!(
            "unexpected valkey response: {other:?}"
        ))),
    }
}
