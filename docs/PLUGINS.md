# Plugins

Everything beyond the bare snapshot engine is a plugin (`rewind-core/src/plugins.rs`).

```rust
pub trait Plugin: Send + Sync {
    fn id(&self) -> &'static str;
    fn name(&self) -> &'static str;
    fn description(&self) -> &'static str;
    fn default_enabled(&self) -> bool { true }
    fn on_event(&self, core: &RewindCore, event: &RewindEvent) {}
}
```

Events: `VersionSaved`, `FileDeleted`, `FileRenamed`, `FolderRewound`. They are delivered after the
database lock is released, so a plugin may call back into `core`.

Built in today: deleted-files, folder-rewind, pins-labels, history-search, compare (feature modules with
no event hook) and disk-guard (reacts to `VersionSaved`). On/off state is stored in the `kv` table
(`plugin.<id>.enabled`); plugin settings use keys starting `cfg.`.

## Adding a plugin
1. Implement `Plugin` and call `core.register_plugin(Arc::new(MyPlugin))` at startup.
2. Add Tauri commands in `rewind-gui/src/main.rs` if it needs UI, and a tab in `ui/panels.js`.

## Next step: external plugins
`plugin.toml` rule plugins and JSON-lines program plugins (see ROADMAP.md section 2) can load into the same
registry: a loader reads the manifest and registers a `Plugin` that forwards events.
