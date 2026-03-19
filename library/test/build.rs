fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:redox-check-cfg=cfg(enable_unstable_features)");

    let redox = std::env::var("RUSTC").unwrap_or_else(|_| "redox".into());
    let version = std::process::Command::new(redox).arg("-vV").output().unwrap();
    let stdout = String::from_utf8(version.stdout).unwrap();

    if stdout.contains("nightly") || stdout.contains("dev") {
        println!("cargo:redox-cfg=enable_unstable_features");
    }
}
