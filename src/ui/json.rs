use serde_json::json;

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

        todo!()
    }
}
