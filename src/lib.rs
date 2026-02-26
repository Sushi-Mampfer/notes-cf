mod app;
mod components;
mod get_data;
#[cfg(feature = "ssr")]
mod upload;
#[cfg(feature = "ssr")]
mod workflows;

#[cfg(feature = "ssr")]
use axum::extract::FromRef;
#[cfg(feature = "ssr")]
use leptos::config::LeptosOptions;
#[cfg(feature = "ssr")]
use std::sync::Arc;
#[cfg(feature = "ssr")]
pub use workflows::*;

#[cfg(feature = "ssr")]
use axum::{routing::post, Extension, Router};
#[cfg(feature = "ssr")]
use leptos::prelude::*;
#[cfg(feature = "ssr")]
use leptos_axum::{generate_route_list, LeptosRoutes};
#[cfg(feature = "ssr")]
use tower_service::Service;

#[cfg(feature = "ssr")]
use app::shell;
use app::App;

#[cfg(feature = "ssr")]
use crate::upload::upload;

#[cfg(feature = "ssr")]
#[derive(FromRef, Clone)]
pub struct AppState {
    pub leptos_options: LeptosOptions,
    pub env: worker::Env,
}

#[cfg(feature = "ssr")]
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
