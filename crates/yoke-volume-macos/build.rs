fn main() {
    // cfg(target_os) in a build script evaluates for the host, not the build
    // target. Read CARGO_CFG_TARGET_OS so cross-compilation stays correct.
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos") {
        println!("cargo:rustc-link-lib=framework=DiskArbitration");
        println!("cargo:rustc-link-lib=framework=IOKit");
        println!("cargo:rustc-link-lib=framework=CoreFoundation");
    }
}
