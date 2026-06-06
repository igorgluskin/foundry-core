//! HTTP router assembly for the application.

mod health;
// Foundry: Phase 3 (multi-project)
mod project;
mod routes;
mod state;
mod trace;

pub use routes::{create_router, create_router_with_all_state, create_router_with_states};
pub use state::{
    ChannelOrchestratorComponents, ModuleStates, build_assistant_state, build_conversation_state,
    build_extension_states, build_module_states, build_ws_state,
};
