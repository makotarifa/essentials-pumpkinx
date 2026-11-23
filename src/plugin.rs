use crate::{
    commands, events, host, tick,
    config::Config,
    extras::Afks,
    logic::{Backs, Cooldowns, Tpas, Warmups},
    store::{HomesFile, WarpsFile, load_homes, load_warps, write_atomic},
};
use pumpkin_plugin_api::{Context, Plugin, PluginMetadata, Result, permissions, register_plugin};
use std::path::PathBuf;
use tracing::{error, info, warn};

struct EssentialsPumpkinX;

impl Plugin for EssentialsPumpkinX {
    fn new() -> Self {
        Self
    }

    fn metadata(&self) -> PluginMetadata {
        PluginMetadata {
            // Intentionally the same name as PumpkinEssentials: the server then
            // exposes the same private data folder, so homes/warps/config are
            // reused verbatim. Remove the old .wasm before installing this one.
            name: "pumpkin_essentials".into(),
            version: env!("CARGO_PKG_VERSION").into(),
            authors: vec!["DarkTama (PumpkinEssentials)".into(), "EssentialsPumpkinX".into()],
            description: "Homes, warps, tpa, back, gamemode, fly, flyspeed and afk.".into(),
            dependencies: vec![],
            permissions: vec![permissions::FS_READ_DATA.into(), permissions::FS_WRITE_DATA.into()],
        }
    }

    fn on_load(&self, context: Context) -> Result<()> {
        let dir = PathBuf::from(context.get_data_folder());

        let cfg_path = dir.join("config.json");
        let cfg = match std::fs::read_to_string(&cfg_path) {
            Ok(text) => Config::from_json(&text).unwrap_or_else(|error| {
                warn!("essentials: bad config.json ({error}); using defaults");
                Config::default()
            }),
            Err(_) => {
                let config = Config::default();
                if let Err(error) = write_atomic(&cfg_path, config.to_json().as_bytes()) {
                    warn!("essentials: could not write default config.json: {error}");
                }
                config
            }
        };
        crate::i18n::set_from_str(&cfg.language);

        // A corrupt data file must not be treated as empty (the next save would destroy it).
        // Legacy shapes are normalised in memory; the tick writes them back.
        let homes: HomesFile = load_homes(&dir.join("homes.json")).map_err(|error| {
            error!("essentials: {error}");
            format!("cannot load homes: {error}")
        })?;
        let persisted: crate::state::PersistedState = crate::state::load_state(&dir.join("state.json")).map_err(|error| {
            error!("essentials: {error}");
            format!("cannot load state: {error}")
        })?;
        let economy = crate::economy::load_economy(&dir.join("economy.json")).map_err(|error| {
            error!("essentials: {error}");
            format!("cannot load economy: {error}")
        })?;
        let kits = crate::kits::load_kits(&dir.join("kits.json")).map_err(|error| {
            error!("essentials: {error}");
            format!("cannot load kits: {error}")
        })?;
        let warps: WarpsFile = load_warps(&dir.join("warps.json")).map_err(|error| {
            error!("essentials: {error}");
            format!("cannot load warps: {error}")
        })?;
        info!(
            "essentials: loaded {} player(s) with homes, {} warp(s)",
            homes.players.len(),
            warps.warps.len()
        );

        host::install(host::State {
            persisted,
            economy,
            cfg,
            kits,
            dir,
            homes,
            warps,
            cooldowns: Cooldowns::default(),
            warmups: Warmups::default(),
            tpas: Tpas::default(),
            backs: Backs::default(),
            afks: Afks::default(),
            last_tick_ms: None,
            pending_confirm: std::collections::HashMap::new(),
        });

        commands::register_all(&context)?;
        events::register_all(&context)?;
        tick::start(&context);
        info!("essentials: EssentialsPumpkinX enabled");
        Ok(())
    }

    fn on_unload(&self, _context: Context) -> Result<()> {
        info!("essentials: EssentialsPumpkinX disabled");
        Ok(())
    }
}

register_plugin!(EssentialsPumpkinX);
