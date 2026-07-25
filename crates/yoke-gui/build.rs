fn main() {
    println!("cargo::rerun-if-changed=assets/icons/app.rc");
    println!("cargo::rerun-if-changed=assets/icons/icon.ico");

    // No-op unless the target is Windows. Without an embedded icon resource the
    // .exe, its taskbar entry, and every shortcut the installer creates fall
    // back to the blank default.
    embed_resource::compile("assets/icons/app.rc", embed_resource::NONE)
        .manifest_optional()
        .expect("compiling assets/icons/app.rc");
}
