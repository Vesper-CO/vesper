fn main() {
    // The real app installer is copied to payload/vesper-setup.exe before building (see README / CI).
    // Without it, we make an empty placeholder so the UI can still be previewed with `tauri dev`.
    let p = std::path::Path::new("payload/vesper-setup.exe");
    if !p.exists() {
        let _ = std::fs::create_dir_all("payload");
        let _ = std::fs::write(p, b"");
        println!("cargo:warning=payload/vesper-setup.exe is missing: this build is a UI-only preview");
    }
    println!("cargo:rerun-if-changed=payload/vesper-setup.exe");
    tauri_build::build()
}
