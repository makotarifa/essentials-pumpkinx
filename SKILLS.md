# Coding skills and conventions

This document describes how EssentialsPumpkinX is organised and the conventions to follow when changing it. The public project name is **EssentialsPumpkinX**. The technical migration contract is deliberately unchanged: `PluginMetadata.name = pumpkin_essentials`, the `pumpkin_essentials:*` permission namespace, and the data folder `plugins/data/pumpkin_essentials/`.

## Project structure

`src/lib.rs` separates pure modules from WASM glue. The pure modules (`state.rs`, `economy.rs`, `kits.rs`, `environment.rs`, `config.rs`, `extras.rs`, `logic.rs`, `store/`) are declared unconditionally: they hold data shapes, migrations, validation, and deterministic domain rules, and they compile and run under `cargo test --lib` on the host. The WASM-only modules (`cmdutil.rs`, `commands/`, `events.rs`, `host.rs`, `plugin.rs`, `tick.rs`) are behind `#[cfg(target_family = "wasm")]`: they call the Pumpkin API, the generated WIT bindings, and the WASI filesystem. About 70% of the code depends on the WIT API, so `cargo check --target wasm32-wasip2` is part of every verification.

- **`src/cmdutil.rs`** — the single copy of shared command-layer helpers: error construction, argument decoding, permission and command-node builders, messaging, and cross-domain helpers. Domain modules import from here instead of redefining utilities.
- **`src/commands/`** — a central registry in `mod.rs` (`register_all`) plus one file per domain: `homes.rs`, `warps.rs`, `tpa.rs`, `movement.rs`, `gamemode.rs`, `info.rs`, `chat.rs`, `moderation.rs`, `economy.rs`, `kits.rs`, `items.rs`, `environment.rs`, and `misc.rs`. Every domain module exports `pub fn register(ctx: &Context) -> Result<(), String>`; `register_all` calls them in order. Shared teleport glue also lives here.
- **`src/store/persist.rs`** — the `FileStore` pattern for `state.json`, `economy.json`, and `kits.json`: read-only load, in-memory normalisation of legacy shapes, canonical schema versions, a migration flag with a deferred write on the first tick, and `save_atomic` (temp file + rename) as the only disk write.
- **`src/host/host_traits.rs`** — minimal traits over the generated Pumpkin WIT bindings. `host.rs` reaches the server through these traits instead of calling `pumpkin-plugin-api` directly, which confines the single WIT implementation to one place. Keep the surface minimal: add a trait method only when an actual caller needs it.

Rules for adding a command:

1. Put the handler in the existing domain file under `src/commands/`; create a new domain module only when no existing one fits. Export `register(ctx: &Context) -> Result<(), String>` and add it to `register_all`.
2. Declare the permission node in the `pumpkin_essentials:` namespace with the correct default level. The loader rejects other namespaces.
3. Use `cmdutil.rs` for arguments, sender/op checks, errors, and replies. Do not duplicate those helpers.
4. Put validation and state transitions in pure modules with unit tests. The handler decodes arguments, calls the pure logic, persists through `FileStore`, and reports through the host adapter.
5. Reuse the existing teleport, chat, and permission glue. Never duplicate a command already provided by the Pumpkin core or another server plugin.
6. Preserve data compatibility: canonical schemas, read-only migration, atomic writes. Unreadable or corrupt JSON must fail the load, never be overwritten.
7. Document the command in `README.md` and update its status in `COMPAT.md` when it belongs to the EssentialsX 2.x inventory.
8. Verify with `cargo test --lib`, `cargo check --target wasm32-wasip2`, and `cargo build --release --target wasm32-wasip2` before installing a build.

## Design patterns

- **Command Handler + Registry** — every command is a small `CommandHandler` implementation; each domain module registers its own nodes and handlers, and `register_all` composes the domains. Handlers stay flat and task-specific; no inheritance trees or handler hierarchies.
- **Repository / FileStore** — one JSON file per store, one canonical schema per store. Loads are read-only, legacy shapes are normalised in memory, the first tick writes the canonical form, and mutation follows clone → mutate → atomic save → commit. This keeps startup non-blocking and makes data loss on a failed write impossible.
- **Adapter over WIT** — narrow traits in `host_traits.rs` with a single implementation over the generated bindings. `host.rs` depends on the traits, so API churn is isolated and host-facing helpers can later be exercised against a mock without touching call sites.
- **Patterns NOT to use** — DI containers or reflection-based wiring; an internal event bus (use direct calls and the existing host events); over-abstraction such as generic factories, one-implementation trait layers, or abstractions for hypothetical future APIs; global mutable state outside `host::with_state`; duplicate command names; silent stubs that report success without doing the work.

## Human-readable code first

- **Descriptive names** — commands, functions, variables, and files say what they do. Abbreviations are limited to established technical terms (WIT, WASI, JSON, UUID, TPS, MSPT, NBT, GUI, TPA).
- **Comments only for the *why*** — document API constraints, migration history, and the reason for a non-obvious choice. Delete comments that merely restate the code.
- **Small functions** — one job per function. Split a function that mixes argument decoding, domain rules, persistence, and messaging.
- **Explicit errors** — return `Result`/`CommandError` with concrete messages. No `unwrap()` on user data, no silent fallback, no fake success; corrupt JSON fails the load.
- **Pure tests** — unit-test the pure modules without WASM, a server, or filesystem side effects. Use table-driven cases for migrations and validation rules.
- **Review checklist** — before merging, confirm that the public name is EssentialsPumpkinX and the technical `pumpkin_essentials` contract is untouched; permission nodes use the required namespace; no command is duplicated; data migration remains read-only with deferred atomic writes; the WIT surface was not widened without a caller; comments explain *why*; errors are explicit; pure tests cover new rules; `README.md` and `COMPAT.md` match the code; and all three verification commands pass.
