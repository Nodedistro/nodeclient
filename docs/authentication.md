# Authentication

Rust modules under packages/minecraft-auth implement each stage independently:

1. Microsoft OAuth: 32 random bytes for state and PKCE verifier; SHA-256 S256 challenge; consumers v2 endpoint; sign-in in an embedded NodeClient window.
2. Code response: matching origin/path, one state, one code, constant-time state comparison, ten-minute expiry. A pending request is consumed once.
3. Xbox Live RPS exchange with d= prefixed Microsoft access token.
4. XSTS RETAIL authorization for rp://api.minecraftservices.com/.
5. Minecraft Services login_with_xbox.
6. mcstore entitlements: require game_minecraft or product_minecraft.
7. Real Minecraft profile, including UUID, skins, and capes.
8. Save only the Microsoft refresh token to OS credentials, keyed by Minecraft UUID; save public profile metadata locally.

Before each Play, rotate the Microsoft refresh token, repeat the entitlement/profile checks, and reject a mismatching account UUID. Authenticated sessions are never returned over IPC. Only the public Profile is serialized to React.

Sign-in opens an in-app Microsoft window. When Microsoft redirects to the configured redirect URI, Tauri intercepts that navigation, validates the response in Rust, and closes the window. No system browser and no manual callback paste are used. No password, client secret, implicit flow, or state-verification bypass exists.

The default redirect is Microsoft's HTTPS nativeclient URI. A registered localhost/127.0.0.1 HTTP redirect with an explicit port is also accepted; the embedded window still intercepts it without a local HTTP listener.

XSTS errors explain missing Xbox profiles, family restrictions, and region issues. HTTP 403 errors explain that Minecraft/Mojang approval may be pending. No response is faked.
