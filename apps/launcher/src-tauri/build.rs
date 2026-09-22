fn main() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../.env");
    println!("cargo:rerun-if-changed={}", path.display());
    // Only the public OAuth configuration is compiled in; never bundle the .env file.
    let values = dotenvy::from_path_iter(&path)
        .ok()
        .map(|i| {
            i.filter_map(Result::ok)
                .collect::<std::collections::HashMap<_, _>>()
        })
        .unwrap_or_default();
    for name in ["MICROSOFT_CLIENT_ID", "MICROSOFT_REDIRECT_URI"] {
        println!("cargo:rerun-if-env-changed={name}");
        if let Some(value) = std::env::var(name)
            .ok()
            .or_else(|| values.get(name).cloned())
        {
            assert!(
                !value.contains(['\r', '\n']),
                "Invalid public OAuth configuration"
            );
            println!("cargo:rustc-env={name}={value}");
        }
    }
    tauri_build::build()
}
