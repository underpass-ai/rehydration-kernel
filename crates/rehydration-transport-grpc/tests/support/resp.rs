use std::error::Error;

use serde_json::Value;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum RespValue {
    BulkString(Option<String>),
    Integer(i64),
}

pub(crate) async fn get_json_value(
    address: &str,
    key: &str,
) -> Result<Value, Box<dyn Error + Send + Sync>> {
    match send_command(address, &["GET", key]).await? {
        RespValue::BulkString(Some(payload)) => Ok(serde_json::from_str(&payload)?),
        other => Err(format!("expected bulk string payload for `{key}`, got {other:?}").into()),
    }
}

pub(crate) async fn get_ttl(address: &str, key: &str) -> Result<i64, Box<dyn Error + Send + Sync>> {
    match send_command(address, &["TTL", key]).await? {
        RespValue::Integer(value) => Ok(value),
        other => Err(format!("expected TTL integer for `{key}`, got {other:?}").into()),
    }
}

async fn send_command(
    address: &str,
    arguments: &[&str],
) -> Result<RespValue, Box<dyn Error + Send + Sync>> {
    let mut stream = TcpStream::connect(address).await?;
    let payload = encode_command(arguments);
    stream.write_all(&payload).await?;
    stream.flush().await?;

    let mut reader = BufReader::new(stream);
    read_response(&mut reader).await
}

fn encode_command(arguments: &[&str]) -> Vec<u8> {
    let mut payload = Vec::new();
    payload.extend_from_slice(format!("*{}\r\n", arguments.len()).as_bytes());
    for argument in arguments {
        payload.extend_from_slice(format!("${}\r\n", argument.len()).as_bytes());
        payload.extend_from_slice(argument.as_bytes());
        payload.extend_from_slice(b"\r\n");
    }

    payload
}

async fn read_response(
    reader: &mut BufReader<TcpStream>,
) -> Result<RespValue, Box<dyn Error + Send + Sync>> {
    let mut line = String::new();
    reader.read_line(&mut line).await?;
    let line = line.trim_end_matches("\r\n");
    let (prefix, remainder) = line.split_at(1);

    match prefix {
        ":" => Ok(RespValue::Integer(remainder.parse()?)),
        "$" => {
            let length: isize = remainder.parse()?;
            if length == -1 {
                return Ok(RespValue::BulkString(None));
            }

            let mut buffer = vec![0_u8; length as usize];
            reader.read_exact(&mut buffer).await?;
            let mut crlf = [0_u8; 2];
            reader.read_exact(&mut crlf).await?;
            Ok(RespValue::BulkString(Some(String::from_utf8(buffer)?)))
        }
        other => Err(format!("unsupported RESP prefix `{other}`").into()),
    }
}
