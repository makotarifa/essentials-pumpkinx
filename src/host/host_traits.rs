//! Minimal adapter over the generated Pumpkin WIT bindings.
//!
//! `host.rs` talks to the server only through these traits instead of calling
//! `pumpkin-plugin-api` directly. This isolates the single WIT implementation
//! (the `impl` blocks below) in one place, so the host-facing helpers in
//! `host.rs` can later be exercised against a mock implementation without
//! touching any call site.
//!
//! The surface is deliberately minimal: only what `host.rs` uses today.
//! Gamemode, health, inventory, permissions, blocks, time and weather are NOT
//! abstracted yet — extend this module when a caller actually needs them.

use pumpkin_plugin_api::{Player, Server, World, text::TextComponent, uuid::Uuid};

/// The world surface used by `host.rs` (identification, teleport targeting).
pub trait HostWorld {
    fn name(&self) -> String;
}

impl HostWorld for World {
    fn name(&self) -> String {
        self.get_name()
    }
}

/// The player surface used by `host.rs`.
pub trait HostPlayer {
    /// The world-handle type this player can be teleported within/between.
    type World: HostWorld;

    fn id(&self) -> Uuid;
    fn entity_id(&self) -> i32;
    fn position(&self) -> (f64, f64, f64);
    /// The player's current (yaw, pitch).
    fn rotation(&self) -> (f32, f32);
    fn world_name(&self) -> String;
    /// Send a system chat message (the plugin always uses overlay=false).
    fn send_system_message(&self, text: TextComponent);
    /// Move the player within their current world.
    fn teleport_same_world(
        &self,
        pos: (f64, f64, f64),
        yaw: Option<f32>,
        pitch: Option<f32>,
        world: Self::World,
    );
    /// Move the player to another world.
    fn teleport_cross_world(
        &self,
        world: Self::World,
        pos: (f64, f64, f64),
        yaw: Option<f32>,
        pitch: Option<f32>,
    );
}

impl HostPlayer for Player {
    type World = World;

    fn id(&self) -> Uuid {
        self.get_id()
    }
    fn entity_id(&self) -> i32 {
        self.as_entity().get_id() as i32
    }
    fn position(&self) -> (f64, f64, f64) {
        self.get_position()
    }
    fn rotation(&self) -> (f32, f32) {
        let entity = self.as_entity();
        (entity.get_yaw(), entity.get_pitch())
    }
    fn world_name(&self) -> String {
        self.get_world().name()
    }
    fn send_system_message(&self, text: TextComponent) {
        Player::send_system_message(self, text, false);
    }
    fn teleport_same_world(
        &self,
        pos: (f64, f64, f64),
        yaw: Option<f32>,
        pitch: Option<f32>,
        world: World,
    ) {
        self.teleport(pos, yaw, pitch, world);
    }
    fn teleport_cross_world(
        &self,
        world: World,
        pos: (f64, f64, f64),
        yaw: Option<f32>,
        pitch: Option<f32>,
    ) {
        self.teleport_world(world, pos, yaw, pitch);
    }
}

/// The server surface used by `host.rs` (world lookup for teleports).
pub trait HostServer {
    type World: HostWorld;

    fn world_by_name(&self, name: &str) -> Option<Self::World>;
}

impl HostServer for Server {
    type World = World;

    fn world_by_name(&self, name: &str) -> Option<World> {
        self.get_world_by_name(name)
    }
}
