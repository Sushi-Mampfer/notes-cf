use wasm_bindgen::prelude::*;
use worker::wasm_bindgen_futures;
use worker::Env;
use workflows_rs::{from_value, WorkflowEvent, WorkflowStep};

#[wasm_bindgen]
pub struct ParseWorkflow {
    env: Env,
}

#[wasm_bindgen]
impl ParseWorkflow {
    #[wasm_bindgen(constructor)]
    pub fn new(_ctx: JsValue, env: Env) -> Self {
        Self { env: env }
    }

    pub async fn run(&self, event: JsValue, step: WorkflowStep) -> Result<String, JsValue> {
        let event: WorkflowEvent<String> = from_value(event).unwrap();
        Ok(event.payload)
    }
}
