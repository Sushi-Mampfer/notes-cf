use std::sync::Arc;

use axum::{
    extract::{Multipart, State},
    http::{Response, StatusCode},
    response::IntoResponse,
};
use base64::{prelude::BASE64_STANDARD, Engine};
use leptos::prelude::*;
use serde::{Deserialize, Serialize};
use workflows_rs::{EnvWorkflowExt, WorkflowInstanceCreateOptions};

use crate::AppState;

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct Input {
    pub name: String,
    pub audio: String,
}

#[derive(Serialize)]
struct WorkflowInput {
    pub id: String,
    pub audio: String,
}

#[axum::debug_handler]
#[worker::send]
pub async fn upload(State(state): State<AppState>, mut multipart: Multipart) -> impl IntoResponse {
    while let Some(field) = multipart.next_field().await.unwrap() {
        let kv = state.env.kv("NOTESKV").unwrap();
        let id = match kv.get("count").text().await.unwrap() {
            Some(n) => n.parse::<i32>().unwrap() + 1,
            None => 0_i32,
        };
        kv.put("count", &id).unwrap().execute().await.unwrap();
        kv.put(&id.to_string(), field.name().unwrap().to_string())
            .unwrap()
            .execute()
            .await
            .unwrap();
        let audio = BASE64_STANDARD.encode(field.bytes().await.unwrap());
        let workflow = state.env.workflow("PARSEWORKFLOW").unwrap();
        workflow
            .create(Some(WorkflowInstanceCreateOptions {
                id: None,
                params: Some(WorkflowInput {
                    id: id.to_string(),
                    audio: audio,
                }),
            }))
            .await
            .unwrap();
    }

    StatusCode::OK
}
