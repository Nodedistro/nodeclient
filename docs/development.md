# Development and acceptance

Run pnpm install, then pnpm tauri dev from the repository root. pnpm dev opens only a browser preview and explicitly disables native operations. All Rust logic lives in workspace crates and can be tested without rendering React.

Checks: pnpm build; pnpm typecheck; cargo check --workspace; pnpm lint; pnpm test. Format Rust with cargo fmt --all. Build the installer with pnpm tauri build with the public Microsoft registration values configured in .env or the build environment.

Core automated coverage includes PKCE RFC vector, state/redirect/duplicate checks, version inheritance, library platform selection, rule ordering, path validation, checksum and size verification, instance memory configuration, Java version parsing, argument substitution, and secret redaction.

Manual acceptance gate (must pass before Fabric work):
1. Start the real desktop app and complete setup.
2. Sign in with Microsoft in the in-app window; confirm the account appears after the window closes.
3. Confirm ownership check succeeds and actual username/UUID appear.
4. Select a version from the official manifest.
5. Install game files and compatible Java, observing real progress.
6. Press Play and verify the Minecraft window opens under that account.
7. Close Minecraft and verify NodeClient returns to Play with recorded playtime.
8. Repeat Play and verify cached-file validation and token refresh.
9. Exercise cancellation, corrupt download recovery, and an invalid custom runtime.

An application approval denial is a real blocking result for steps 3–8. Document it rather than bypassing entitlement checks or using an offline profile.
