//! Build script — the Windows icon embedded in the .exe.

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=assets/icon.ico");

    // Embeds the icon in the .exe (Start menu, shortcuts, Explorer). The
    // release workflow makes assets/icon.ico from assets/icon.png just before
    // building; without it the build still succeeds, icon-less.
    //
    // Under id 32512 (IDI_APPLICATION): Dioxus sets the window icon — what the
    // taskbar and title bar show — from that resource, and falls back to its
    // own logo when the .exe has none there.
    #[cfg(target_os = "windows")]
    {
        if std::path::Path::new("assets/icon.ico").exists() {
            let mut res = winresource::WindowsResource::new();
            res.set_icon_with_id("assets/icon.ico", "32512");
            res.set("FileDescription", "JobScout");
            res.set("ProductName", "JobScout");
            res.set("CompanyName", "Mayorana");
            res.set("LegalCopyright", "© 2026 Mayorana");
            if let Err(e) = res.compile() {
                println!(
                    "cargo:warning=Failed to embed the Windows icon: {e} (building without it)"
                );
            }
        } else {
            println!("cargo:warning=assets/icon.ico not found — the .exe will ship without an embedded icon.");
        }
    }
}
