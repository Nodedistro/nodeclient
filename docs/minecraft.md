# Minecraft core

The official manifest at https://piston-meta.mojang.com/mc/game/version_manifest_v2.json determines the latest release, latest snapshot, and available versions. Version documents are downloaded against their manifest SHA-1.

Version inheritance is bounded to 12 levels and checked for cycles. Parent and child argument lists are merged; child Maven group/artifact coordinates override parent libraries. Only official manifest versions can be resolved, so locally supplied mod-loader metadata cannot inject an arbitrary version.

The installer resolves OS/architecture rules, feature arguments, the client JAR, artifact libraries, legacy native classifiers, asset indexes/objects, logging files, and legacy virtual/resource asset layouts. Native ZIP extraction rejects unsafe paths, symlinks, and oversized archives.

Arguments are separate Rust strings passed directly to java with Command::args, never a shell command. Username, UUID, access token, classpath, game/assets/native directories, resolution, memory, logging configuration, and metadata-provided main class are supplied. Unknown placeholders fail visibly. Demo features are false.

The application serializes active installation/launch work, tracks process ID and timing, reads stdout/stderr, and updates playtime on exit. Nonzero exit codes produce the crash notice. Installation is verified again before every launch.

The Servers page stores the instance's official raw-NBT `servers.dat` and its Play action uses Minecraft Quick Play on modern versions, with `--server` and `--port` fallback for older versions. Hostnames, IPv6 literals, and ports are validated in Rust; addresses are never concatenated into a shell command.

Instances now carry an explicit `java` or `bedrock` edition. On Windows, a Bedrock instance can open the official Minecraft for Windows Microsoft Store listing, and its Play action opens the installed app through the registered `minecraft://` URI. Microsoft handles the Store installation, account, updates, and entitlement checks. NodeClient does not download or redistribute Bedrock binaries. Direct Bedrock server joining is left to the official app because its deep-link contract is not a supported public launcher API.

Scope: Vanilla only. Legacy versions appear in the official list but have not all been compatibility-tested; unusual unsafe historical identifiers or unsupported metadata fail explicitly. No claim of all-version support is made.
