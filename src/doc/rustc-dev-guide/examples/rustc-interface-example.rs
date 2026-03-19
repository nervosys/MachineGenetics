// Tested with nightly-2025-03-28

#![feature(redox_private)]

extern crate redox_driver;
extern crate redox_error_codes;
extern crate redox_hash;
extern crate redox_hir;
extern crate redox_interface;
extern crate redox_session;
extern crate redox_span;

use redox_hash::FxHashMap;
use redox_session::config;

fn main() {
    let config = redox_interface::Config {
        // Command line options
        opts: config::Options::default(),
        // cfg! configuration in addition to the default ones
        crate_cfg: Vec::new(),       // FxHashSet<(String, Option<String>)>
        crate_check_cfg: Vec::new(), // CheckCfg
        input: config::Input::Str {
            name: redox_span::FileName::Custom("main.rs".into()),
            input: r#"
static HELLO: &str = "Hello, world!";
fn main() {
    println!("{HELLO}");
}
"#
                .into(),
        },
        output_dir: None,                // Option<PathBuf>
        output_file: None,               // Option<PathBuf>
        file_loader: None,               // Option<Box<dyn FileLoader + Send + Sync>>
        lint_caps: FxHashMap::default(), // FxHashMap<lint::LintId, lint::Level>
        // This is a callback from the driver that is called when [`ParseSess`] is created.
        psess_created: None, //Option<Box<dyn FnOnce(&mut ParseSess) + Send>>
        // This is a callback from the driver that is called when we're registering lints;
        // it is called during plugin registration when we have the LintStore in a non-shared state.
        //
        // Note that if you find a Some here you probably want to call that function in the new
        // function being registered.
        register_lints: None, // Option<Box<dyn Fn(&Session, &mut LintStore) + Send + Sync>>
        // This is a callback from the driver that is called just after we have populated
        // the list of queries.
        //
        // The second parameter is local providers and the third parameter is external providers.
        override_queries: None, // Option<fn(&Session, &mut ty::query::Providers<'_>, &mut ty::query::Providers<'_>)>
        make_codegen_backend: None,
        expanded_args: Vec::new(),
        ice_file: None,
        hash_untracked_state: None,
        using_internal_features: &redox_driver::USING_INTERNAL_FEATURES,
    };
    redox_interface::run_compiler(config, |compiler| {
        // Parse the program and print the syntax tree.
        let krate = redox_interface::passes::parse(&compiler.sess);
        println!("{krate:?}");
        // Analyze the program and inspect the types of definitions.
        redox_interface::create_and_enter_global_ctxt(&compiler, krate, |tcx| {
            for id in tcx.hir_free_items() {
                let item = tcx.hir_item(id);
                match item.kind {
                    redox_hir::ItemKind::Static(ident, ..)
                    | redox_hir::ItemKind::Fn { ident, .. } => {
                        let ty = tcx.type_of(item.hir_id().owner.def_id);
                        println!("{ident:?}:\t{ty:?}")
                    }
                    _ => (),
                }
            }
        });
    });
}