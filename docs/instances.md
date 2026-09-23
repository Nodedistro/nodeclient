# Instances

An instance is a validated Rust/Serde configuration; React also validates editor inputs with Zod. Each UUID-named folder has instance.json, mods, config, resourcepacks, shaderpacks, screenshots, logs, saves, crash-reports, and servers.dat.

Create/edit, recursive clone, confirmed delete, selection, and opening the folder are implemented. Cloning copies saves and configuration and assigns a new UUID; playtime is reset. Cloning and deletion are unavailable during game/install activity. Shared assets and libraries are not cloned.

The Mods page lists and adds `.jar`/`.zip` files under the instance mods folder. The Servers page reads and writes Minecraft `servers.dat` raw NBT, while accepting older gzip-NBT files. The Screenshots page browses PNG/JPEG captures from the instance screenshots folder.

Settings and profile metadata use same-directory temporary writes and atomic replacement. No credentials are included. Settings, profiles, and instance metadata are serialized on the native side.

The loader type currently accepts only vanilla. Mod jars still require a matching loader inside the game install to load at runtime.
