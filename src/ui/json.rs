use std::io::{BufRead, BufReader};

use serde_json::json;
use tracing::error;

use crate::errors::Result;

use super::DebuggerUI;

pub struct JsonUI {}

impl JsonUI {
    pub fn build() -> Result<Self> {
        Ok(JsonUI {})
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
            match serde_json::from_slice(&buf) {
                Ok(a) => return Ok(a),
                Err(e) => {
                    error!("{e}");
                    continue;
                }
            }
        }
    }
}
