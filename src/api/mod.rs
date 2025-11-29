pub mod auth;
pub mod handlers;
pub mod websocket;

use axum::{
    routing::{delete, get, post, put},
    Router,
};
use std::sync::Arc;
use tower_http::cors::CorsLayer;

use crate::proxy::ProxyServer;

pub fn create_router(proxy: Arc<ProxyServer>) -> Router {
    Router::new()
        // Index and help
        .route("/", get(handlers::index))
        .route("/filter-help", get(handlers::filter_help))
        .route("/filter-help.json", get(handlers::filter_help))

        // WebSocket updates
        .route("/updates", get(handlers::websocket_handler))

        // Commands
        .route("/commands", get(handlers::get_commands))
        .route("/commands.json", get(handlers::get_commands))
        .route("/commands/:cmd", post(handlers::execute_command))

        // Events
        .route("/events", get(handlers::get_events))
        .route("/events.json", get(handlers::get_events))

        // Flows
        .route("/flows", get(handlers::get_flows))
        .route("/flows.json", get(handlers::get_flows))
        .route("/flows/dump", get(handlers::dump_flows).post(handlers::load_flows))
        .route("/flows/resume", post(handlers::resume_flows))
        .route("/flows/kill", post(handlers::kill_flows))

        // Individual flow operations
        .route("/flows/:flow_id",
               get(handlers::get_flow)
               .put(handlers::update_flow)
               .delete(handlers::delete_flow))
        .route("/flows/:flow_id/resume", post(handlers::resume_flow))
        .route("/flows/:flow_id/kill", post(handlers::kill_flow))
        .route("/flows/:flow_id/duplicate", post(handlers::duplicate_flow))
        .route("/flows/:flow_id/replay", post(handlers::replay_flow))
        .route("/flows/:flow_id/revert", post(handlers::revert_flow))

        // Flow content
        .route("/flows/:flow_id/:message/content.data",
               get(handlers::get_flow_content)
               .post(handlers::set_flow_content))
        .route("/flows/:flow_id/:message/content/:content_view",
               get(handlers::get_flow_content_view))
        .route("/flows/:flow_id/:message/content/:content_view.json",
               get(handlers::get_flow_content_view))

        // Clear all
        .route("/clear", post(handlers::clear_all))

        // Options
        .route("/options", get(handlers::get_options).put(handlers::set_options))
        .route("/options.json", get(handlers::get_options))
        .route("/options/save", post(handlers::save_options))

        // State
        .route("/state", get(handlers::get_state))
        .route("/state.json", get(handlers::get_state))

        // Process information
        .route("/processes", get(handlers::get_processes))
        .route("/executable-icon", get(handlers::get_executable_icon))

        .layer(CorsLayer::permissive())
        .with_state(proxy)
}