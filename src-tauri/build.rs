fn main() {
    let windows_msvc = std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows")
        && std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc");

    let mut windows = tauri_build::WindowsAttributes::new();
    if windows_msvc {
        // tauri-build only links its app manifest into binaries. Test
        // executables also link Tauri, and without the Common Controls v6
        // manifest Windows refuses to start them (STATUS_ENTRYPOINT_NOT_FOUND).
        // Embed the same manifest through the linker for every artifact instead.
        windows = tauri_build::WindowsAttributes::new_without_app_manifest();
        let manifest =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("windows-app-manifest.xml");
        println!("cargo:rerun-if-changed={}", manifest.display());
        println!("cargo:rustc-link-arg=/MANIFEST:EMBED");
        println!("cargo:rustc-link-arg=/MANIFESTINPUT:{}", manifest.display());
    }

    if let Err(error) =
        tauri_build::try_build(tauri_build::Attributes::new().windows_attributes(windows))
    {
        panic!("tauri-build failed: {error:#}");
    }
}
