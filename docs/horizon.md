# Horizon native instances

NodeClient is owned and maintained by Nodedistro. Horizon is a separate upstream Rust engine, used under its MIT license; Nodedistro does not claim ownership of Horizon or Minecraft.

Create an instance and choose **Horizon · Native Rust**. Existing instances remain Java by default. Engines cannot be switched on an existing instance, preserving its worlds and configuration. Horizon currently targets Minecraft **26.3**, pinned to `nightly-2026-11-13-9f42064` and Launch API `0.1.0`, on Windows x64.

Sign in through NodeClient before preparing or playing. Launcher startup restores and verifies saved accounts once. Launch uses the in-memory session, checks expiration and sign-out locally, and sends the token through a private stdin pipe. Tokens are excluded from command arguments and process history. An expired session requires reopening NodeClient or signing in again.

The first installation verifies the pinned engine archive SHA-256 and executable hashes, downloads official Minecraft resources, and uses Java 25 to prepare them. Gameplay runs in Horizon's native executable. Preparation can take time and needs disk space and an internet connection. The launcher preserves the upstream archive's license and third-party notices. Subsequent launches check the prepared resources. Repair repeats resource preparation; it does not delete worlds.

Each instance has its own game and home directory. Java worlds are not automatically imported. Java Fabric, Quilt, Forge, and NeoForge loaders and Modrinth Java mods are blocked for native instances; use a Java instance for those. The Java safe-mode toggle does not apply to Horizon.

Horizon remains experimental. This integration does not establish complete vanilla behavior, server compatibility, or Java mod compatibility. Its native content/mod ecosystem is not yet managed by NodeClient. The original standalone NodeEngine prototype is separate; see [native-engine.md](native-engine.md).

## Validation

`cargo test -p nodeclient-horizon -p nodeclient-profiles -p nodeclient-process` checks API contracts, pinned download boundaries, instance isolation, loader rejection, session stdin and log redaction. `cargo run -p nodeclient-horizon --example verify_runtime -- .verification-data/horizon-adapter` downloads and checks the actual pinned binaries without accessing an account. `verify_account` checks a locally saved account without printing credentials; it may rotate its refresh token through the normal authentication code.

Pass an isolated data directory to `verify_account` to install and prepare owned resources. With those resources already prepared, append `--launch-smoke` to start the native client with a verified session for 15 seconds and stop it. The smoke check detects an early process exit; it does not assess rendering or gameplay.

Verified locally on Windows: pinned binary download and API checks, saved-account ownership verification, official Minecraft resource installation, Java 25 preparation, and Horizon's resource completeness check (including 4,962 sound files). The authenticated native client stayed running for the full 15-second startup smoke test and was then stopped. Visual gameplay, multiplayer, and long-running world persistence remain unverified. The frontend production build and launcher release build passed. A debug stack overflow in nested download futures was fixed by allocating the checksum buffer on the heap.

An unavailable pinned release is an explicit error. Updating the engine requires reviewing the new upstream Launch API and replacing the build, archive size, asset ID, and hashes together; moving nightly downloads are not trusted automatically.

## Attribution

[Horizon source and license](https://github.com/horizon-rust/horizon): MIT, Copyright (c) 2026 Ludovic Marie and Horizon contributors. See its [Launch API](https://github.com/horizon-rust/horizon/blob/main/docs/api/launch.md). Downloaded engine distributions retain their original `LICENSE` and dependency notices, including DirectX Shader Compiler notices. Minecraft resources are obtained separately and are not included in NodeClient's source or installer.
