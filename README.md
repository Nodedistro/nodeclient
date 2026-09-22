# NodeClient

An original desktop launcher for Minecraft Java Edition, built with Tauri 2, Rust, React, TypeScript, Vite, Tailwind CSS, and shadcn/ui. No launcher backend, telemetry, offline accounts, or bundled Minecraft files.

The first milestone is Vanilla: Microsoft sign-in → ownership/profile → official version selection → verified installation → compatible Java → real Minecraft process. Fabric, mods, server management, screenshot gallery, and updates are deliberately deferred until that full flow has been verified.

## Requirements

- Windows 11 x64 (primary development target), Microsoft Edge WebView2.
- Node.js 22+ and pnpm 11.19.0.
- Rust stable (tested toolchain: 1.98.1), Visual Studio C++ Build Tools and Windows SDK.
- A Microsoft personal account with Minecraft Java Edition.
- Internet access to Microsoft, Xbox, and official Mojang/Minecraft services.

Java need not already be installed: automatic mode can install Mojang's hash-verified Windows x64 runtime. Other platforms can use a local Java runtime, but have not been validated.

## Get started

```powershell
pnpm install
Copy-Item .env.example .env
# Edit .env locally before running the desktop application.
pnpm tauri dev
```

Do not overwrite an existing configured `.env`. Place your application ID in the **repository root** `.env`:

```dotenv
MICROSOFT_CLIENT_ID=your-application-client-id
MICROSOFT_REDIRECT_URI=https://login.microsoftonline.com/common/oauth2/nativeclient
```

The client ID is a public application identifier; no client secret is used. Never place access or refresh tokens in this file. It is ignored by Git and is not exposed through Vite.

### Microsoft registration

Use a **Mobile and desktop applications** platform registration that permits **personal Microsoft accounts**. The redirect must match the registration. NodeClient uses Authorization Code + PKCE and `XboxLive.signin offline_access`.

Sign-in opens an in-app Microsoft window. When Microsoft redirects to the configured URI, NodeClient intercepts the response, validates the single-use code and state in Rust, and completes Xbox/Minecraft authentication. No system browser or callback paste is required. The default nativeclient redirect is recommended; a registered localhost/127.0.0.1 HTTP URI with an explicit port also works with the same intercept path.

Minecraft Services access may require Microsoft/Mojang application review. A 403 is reported as a configuration/approval issue, not treated as successful authentication. Development, metadata browsing, and game installation can proceed without account approval. Playing requires fresh authentication, entitlement verification, and the real profile.

## Commands

| Command | Purpose |
| --- | --- |
| `pnpm install` | Install frontend/CLI dependencies |
| `pnpm dev` | Browser UI preview; desktop commands unavailable |
| `pnpm tauri dev` | Real desktop app |
| `pnpm build` | TypeScript check and production frontend build |
| `pnpm typecheck` | Strict TypeScript check |
| `pnpm lint` | ESLint and React hooks rules |
| `pnpm test` | Frontend and Rust unit tests |
| `cargo check --workspace` | Check all Rust packages |
| `pnpm tauri build` | Build Windows executable and NSIS installer |

For release builds, build.rs reads the root .env or build environment and embeds only these two public configuration values (environment takes precedence). A release executable does not read the development repository's `.env`.

## Layout

- `apps/launcher`: React interface; `src-tauri` is the native command boundary.
- `packages/minecraft-auth`: separate Microsoft, Xbox, XSTS, Minecraft, entitlements, profile, secure-store, and auth-manager Rust modules. They are intentionally not TypeScript: secrets never need to cross into React.
- `packages/minecraft-core`: version metadata/inheritance, OS rules, installation, argument construction.
- `packages/downloader`: parallel, cancellable, verified downloads.
- `packages/java-manager`: local detection and managed official runtime installation.
- `packages/profile-manager`: validated local instances, profiles, and settings.
- `packages/process-manager`: process creation, lifetime, output capture, and redaction.
- `packages/shared-types`: Rust models and filesystem/security validation.
- `apps/launcher/src/components/ui`: locally owned shadcn/ui source.

## Data and security

Data uses Tauri's per-user application-data directory for `app.nodeclient.launcher`, normally `%APPDATA%\\app.nodeclient.launcher`. Each instance has its own saves, config, mods, resource packs, shader packs, screenshots, and logs. Shared libraries/assets/runtimes are cached under the same managed root.

Refresh credentials use Windows Credential Manager (macOS Keychain / Linux Secret Service through the same abstraction). Access tokens are transient Rust values. Game output is redacted before NodeClient writes or displays it. Minecraft itself necessarily receives its access token in its process arguments; do not share OS-level process dumps. Custom Java selection runs the selected executable to validate it, so choose only a runtime you trust.

See [security](docs/security.md), [authentication](docs/authentication.md), [development and acceptance tests](docs/development.md), and [verification record](docs/verification.md).

NodeClient is independent and is not an official Minecraft product or associated with Mojang or Microsoft.
