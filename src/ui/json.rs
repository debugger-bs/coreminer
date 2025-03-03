use std::io::{BufRead, BufReader};

use serde::{Deserialize, Serialize};
use serde_json::json;
use tracing::error;

use crate::errors::Result;
use crate::feedback::Feedback;

use super::{DebuggerUI, Status};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Input {
    pub status: Status,
}

pub struct JsonUI {}

impl JsonUI {
    pub fn build() -> Result<Self> {
        Ok(JsonUI {})
    }

    pub fn format_feedback(&self, feedback: &Feedback) -> Result<serde_json::Value> {
        Ok(json!({ "feedback": feedback }))
    }
}

impl DebuggerUI for JsonUI {
    fn process(&mut self, feedback: crate::feedback::Feedback) -> Result<super::Status> {
        println!("{}", json!({ "feedback": feedback }));

        let mut reader = BufReader::new(std::io::stdin());
        let mut buf = Vec::new();
        loop {
            buf.clear();
            reader.read_until(b'\n', &mut buf)?;
            let input: Input = match serde_json::from_slice(&buf) {
                Ok(a) => a,
                Err(e) => {
                    error!("{e}");
                    continue;
                }
            };
            return Ok(input.status);
        }
    }
}
