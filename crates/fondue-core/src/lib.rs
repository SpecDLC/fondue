//! fondue-core: an event-sourced recipe graph.
//!
//! - `model`   — one node type (concept, item, preparation, gathering), slots, canons, contracts
//! - `state`   — the event log and the projection folded from it
//! - `resolve` — profile-aware resolution, contract checks, deltas, folding, lockfile, portal, diff
//! - `api`     — JSON command surface used by the WASM shell and tests

pub mod api;
pub mod model;
pub mod resolve;
pub mod state;

/// The demo seed: butter to mac and cheese to a Sunday dinner.
pub const SEED: &str = include_str!("../../../seed/events.json");

pub fn seed_events() -> Vec<state::Event> {
    serde_json::from_str(SEED).expect("seed/events.json must parse")
}
