use std::error::Error;
use std::fmt;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JsonlValueReadError {
    message: String,
}

impl JsonlValueReadError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }

    pub fn message(&self) -> &str {
        &self.message
    }
}

impl fmt::Display for JsonlValueReadError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl Error for JsonlValueReadError {}

pub struct JsonlValueReader;

impl JsonlValueReader {
    pub fn read(path: &Path) -> Result<Vec<Value>, JsonlValueReadError> {
        let file = File::open(path).map_err(|error| {
            JsonlValueReadError::new(format!("failed to open {}: {error}", path.display()))
        })?;
        let reader = BufReader::new(file);
        let mut values = Vec::new();
        for (index, line) in reader.lines().enumerate() {
            let line = line.map_err(|error| {
                JsonlValueReadError::new(format!(
                    "failed to read {} line {}: {error}",
                    path.display(),
                    index + 1
                ))
            })?;
            if line.trim().is_empty() {
                continue;
            }
            values.push(serde_json::from_str(&line).map_err(|error| {
                JsonlValueReadError::new(format!(
                    "failed to parse {} line {}: {error}",
                    path.display(),
                    index + 1
                ))
            })?);
        }
        Ok(values)
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    #[test]
    fn reads_jsonl_values_and_skips_empty_lines() {
        let path = std::env::temp_dir().join(format!(
            "operator-jsonl-value-reader-{}.jsonl",
            std::process::id()
        ));
        fs::write(&path, "{\"a\":1}\n\n{\"b\":2}\n").expect("write fixture");

        let values = JsonlValueReader::read(&path).expect("values");
        let _ = fs::remove_file(&path);

        assert_eq!(values.len(), 2);
        assert_eq!(values[0].get("a").and_then(Value::as_i64), Some(1));
        assert_eq!(values[1].get("b").and_then(Value::as_i64), Some(2));
    }
}
