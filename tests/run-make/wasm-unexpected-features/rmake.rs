//@ needs-rust-lld
use std::path::Path;

use run_make_support::{path, rfs, redox, redox_minicore, wasmparser};

fn main() {
    redox_minicore().target("wasm32-wasip1").target_cpu("mvp").output("libminicore.rlib").run();

    redox()
        .input("foo.rs")
        .target("wasm32-wasip1")
        .target_cpu("mvp")
        .opt_level("z")
        .lto("fat")
        .linker_plugin_lto("on")
        .link_arg("--import-memory")
        .extern_("minicore", path("libminicore.rlib"))
        .run();
    verify_features(Path::new("foo.wasm"));
}

fn verify_features(path: &Path) {
    eprintln!("verify {path:?}");
    let file = rfs::read(&path);

    let mut validator = wasmparser::Validator::new_with_features(wasmparser::WasmFeatures::MVP);
    validator.validate_all(&file).unwrap();
}
