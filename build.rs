fn main() {
    // On Windows, embed the application icon into the .exe so Explorer and
    // the taskbar show the same icon as the macOS .app bundle.
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        #[cfg(windows)]
        {
            let mut resource = winres::WindowsResource::new();
            resource.set_icon("resources/AppIcon.ico");
            if let Err(error) = resource.compile() {
                println!("cargo:warning=failed to embed Windows icon: {error}");
            }
        }
    }
}
