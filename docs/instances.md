# Instances

An instance is a validated Rust/Serde configuration; React also validates editor inputs with Zod. Each UUID-named folder has instance.json, mods, config, resourcepacks, shaderpacks, screenshots, logs, saves, and crash-reports.

Create/edit, recursive clone, confirmed delete, selection, and opening the folder are implemented. Cloning copies saves and configuration and assigns a new UUID; playtime is reset. Cloning and deletion are unavailable during game/install activity. Shared assets and libraries are not cloned.

Settings and profile metadata use same-directory temporary writes and atomic replacement. No credentials are included. Settings, profiles, and instance metadata are serialized on the native side.

The loader type currently accepts only vanilla. Extending the loader interface should happen after a successful authenticated Vanilla acceptance run.
