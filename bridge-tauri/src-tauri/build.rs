fn main() {
    println!("cargo:rerun-if-changed=windows-app-manifest.xml");
    let manifest = include_str!("windows-app-manifest.xml");
    let windows = tauri_build::WindowsAttributes::new().app_manifest(manifest);
    if let Err(e) = tauri_build::try_build(tauri_build::Attributes::new().windows_attributes(windows)) {
        panic!("{e:#}");
    }
}
