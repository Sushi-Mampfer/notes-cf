#![cfg(feature = "ssr")]
mod app;
mod components;
mod get_data;
mod upload;
mod workflows;

use axum::extract::FromRef;
use axum::{routing::post, Extension, Router};
use leptos::config::LeptosOptions;
use leptos::prelude::*;
use leptos_axum::{generate_route_list, LeptosRoutes};
use std::sync::Arc;
use tower_service::Service;
pub use workflows::*;

use app::shell;
use app::App;

use crate::upload::upload;

#[derive(FromRef, Clone)]
pub struct AppState {
    pub leptos_options: LeptosOptions,
    pub env: worker::Env,
}

#[worker::event(fetch)]
async fn fetch(
    req: worker::HttpRequest,
    env: worker::Env,
    _ctx: worker::Context,
) -> worker::Result<axum::http::Response<axum::body::Body>> {
    use axum::extract::DefaultBodyLimit;

    let conf = get_configuration(None).unwrap();
    let leptos_options = conf.leptos_options;
    let routes = generate_route_list(App);
    let state = AppState {
        leptos_options: leptos_options.clone(),
        env: env.clone(),
    };

    // build our application with a route
    let mut router = Router::new()
        .route("/upload", post(upload))
        .leptos_routes(&state, routes, {
            let leptos_options = leptos_options.clone();
            move || shell(leptos_options.clone())
        })
        .with_state(state)
        .layer(Extension(Arc::new(env))) // <- Allow leptos server functions to access Worker stuff
        .layer(DefaultBodyLimit::disable());

    Ok(router.call(req).await?)
}

#[cfg(feature = "hydrate")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn hydrate() {
    leptos::mount::hydrate_body(app::App);
}
