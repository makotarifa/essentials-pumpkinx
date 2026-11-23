//! EssentialsFusion: one WASM plugin combining PumpkinEssentials and VEssentials features.
//!
//! Pure, host-testable modules: `config`, `extras`, `logic`, `store`.
//! WASM-only glue: `host`, `commands`, `events`, `tick`, `plugin`.
//!
//! The plugin metadata name is deliberately `pumpkin_essentials` so that the
//! existing `plugins/data/pumpkin_essentials/` files are read and written
//! directly: no data copy or conversion on install is required.
pub mod state;
pub mod economy;
pub mod kits;
pub mod environment;
pub mod config;
pub mod extras;
pub mod logic;
pub mod store;

#[cfg(target_family = "wasm")]
mod commands;
#[cfg(target_family = "wasm")]
mod events;
#[cfg(target_family = "wasm")]
mod host;
#[cfg(target_family = "wasm")]
mod iteration4;
#[cfg(target_family = "wasm")]
mod iteration5;
#[cfg(target_family = "wasm")]
mod iteration6;
#[cfg(target_family = "wasm")]
mod iteration7;
#[cfg(target_family = "wasm")]
mod iteration8;
#[cfg(target_family = "wasm")]
mod iteration9;
#[cfg(target_family = "wasm")]
mod plugin;
#[cfg(target_family = "wasm")]
mod tick;
