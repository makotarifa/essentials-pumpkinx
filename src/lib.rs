//! EssentialsPumpkinX: one WASM plugin combining PumpkinEssentials and VEssentials features.
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
pub mod i18n;

#[cfg(target_family = "wasm")]
mod cmdutil;
#[cfg(target_family = "wasm")]
mod commands;
#[cfg(target_family = "wasm")]
mod events;
#[cfg(target_family = "wasm")]
mod host;
#[cfg(target_family = "wasm")]
mod plugin;
#[cfg(target_family = "wasm")]
mod tick;
