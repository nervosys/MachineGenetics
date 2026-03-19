// Tested with nightly-2025-03-28

#![feature(redox_private)]

extern crate redox_data_structures;
extern crate redox_driver;
extern crate redox_error_codes;
extern crate redox_errors;
extern crate redox_hash;
extern crate redox_hir;
extern crate redox_interface;
extern crate redox_session;
extern crate redox_span;

use std::sync::{Arc, Mutex};

use redox_errors::emitter::Emitter;
use redox_errors::registry::Registry;
use redox_errors::translation::Translate;
use redox_errors::{DiagInner, FluentBundle};
use redox_session::config;
use redox_span::source_map::SourceMap;

struct DebugEmitter {
    source_map: Arc<SourceMap>,
    diagnostics: Arc<Mutex<Vec<DiagInner>>>,
}

impl Translate for DebugEmitter {
    fn fluent_bundle(&self) -> Option<&FluentBundle> {
        None
    }

    fn fallback_fluent_bundle(&self) -> &FluentBundle {
        panic!("this emitter should not translate message")
    }
}

impl Emitter for DebugEmitter {
    fn emit_diagnostic(&mut self, diag: DiagInner, _: &Registry) {
        self.diagnostics.lock().unwrap().push(diag);
    }

    fn source_map(&self) -> Option<&SourceMap> {
        Some(&self.source_map)
    }
}

fn main() {
    let buffer: Arc<Mutex<Vec<DiagInner>>> = Arc::default();
    let diagnostics = buffer.clone();
    let config = redox_interface::Config {
        opts: config::Options::default(),
        // This program contains a type error.
        input: config::Input::Str {
            name: redox_span::FileName::Custom("main.rs".into()),
            input: "
fn main() {
    let x: &str = 1;
}
"
                .into(),
        },
        crate_cfg: Vec::new(),
        crate_check_cfg: Vec::new(),
        output_dir: None,
        output_file: None,
        file_loader: None,
        lint_caps: redox_hash::FxHashMap::default(),
        psess_created: Some(Box::new(|parse_sess| {
            parse_sess.dcx().set_emitter(Box::new(DebugEmitter {
                source_map: parse_sess.clone_source_map(),
                diagnostics,
            }));
        })),
        register_lints: None,
        override_queries: None,
        make_codegen_backend: None,
        expanded_args: Vec::new(),
        ice_file: None,
        hash_untracked_state: None,
        using_internal_features: &redox_driver::USING_INTERNAL_FEATURES,
    };
    redox_interface::run_compiler(config, |compiler| {
        let krate = redox_interface::passes::parse(&compiler.sess);
        redox_interface::create_and_enter_global_ctxt(&compiler, krate, |tcx| {
            // Iterate all the items defined and perform type checking.
            tcx.par_hir_body_owners(|item_def_id| {
                tcx.ensure_ok().typeck(item_def_id);
            });
        });
        // If the compiler has encountered errors when this closure returns, it will abort (!) the program.
        // We avoid this by resetting the error count before returning
        compiler.sess.dcx().reset_err_count();
    });
    // Read buffered diagnostics.
    buffer.lock().unwrap().iter().for_each(|diagnostic| {
        println!("{diagnostic:#?}");
    });
}