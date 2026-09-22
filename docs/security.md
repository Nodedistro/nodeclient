# Security boundaries

- Public desktop OAuth client, S256 PKCE, unpredictable state, strict redirect/state checking, no client secret or password handling.
- OS secure storage only for refresh tokens; public profile data is separate. Tokens never enter localStorage or JSON.
- Rust performs HTTPS requests, filesystem writes, archive extraction, and process spawning. React receives profiles, settings, status, and redacted logs.
- Download hosts are explicitly allowed; hashes anchor game/runtime content to official metadata. TLS verification and integrity checks cannot be disabled.
- Managed paths reject absolute paths, traversal, Windows device names, alternate streams, backslashes, and symlink/reparse-point ancestors. Native archives reject links and enforce size/count limits.
- Custom JVM flags use a small tuning allowlist. Java environment injection variables are removed before launch. No shell concatenation.
- A launcher operation guard rejects repeated launches and instance deletion during activity.
- Deletion requires an explicit confirmation UI and a native confirmed argument.
- The WebView CSP disallows remote scripts and only permits Minecraft texture images. There is no frontend shell or unrestricted filesystem plugin.
- No telemetry or mandatory NodeClient backend.
- Updates check `Nodedistro/nodeclient` GitHub Releases over HTTPS, require a SHA-256 checksum from the release digest or notes, download only from GitHub asset hosts, verify the hash before launch, and open the NSIS installer for the user to finish. Silent in-place replacement and unsigned updates are not supported.

Limits: same-user malicious processes can race filesystem checks or inspect process memory/arguments; this is not an OS sandbox. Minecraft receives its token as required by its launch protocol. NodeClient redacts captured output and diagnostics, but cannot control every file produced internally by a third-party Java runtime/game. Do not publish raw crash dumps or process command lines.

A security review and signed release process are still required before general distribution.
