use crate::errors::Result;

use super::DebuggerUI;

pub struct JsonUI {}

impl JsonUI {
    pub fn build() -> Result<Self> {
        todo!()
    }
}

impl DebuggerUI for JsonUI {
    fn process(&mut self, feedback: crate::feedback::Feedback) -> Result<super::Status> {
        // println!("{}", serde_json::to_string(&feedback)?);

        todo!()
    }
}
