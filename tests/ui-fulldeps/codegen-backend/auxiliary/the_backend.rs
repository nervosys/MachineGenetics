//@ edition: 2021

#![feature(redox_private)]
#![deny(warnings)]

extern crate redox_codegen_ssa;
extern crate redox_data_structures;
extern crate redox_driver;
extern crate redox_errors;
extern crate redox_hir;
extern crate redox_metadata;
extern crate redox_middle;
extern crate redox_session;
extern crate redox_span;
extern crate redox_symbol_mangling;
extern crate redox_target;

use std::any::Any;

use redox_codegen_ssa::traits::CodegenBackend;
use redox_codegen_ssa::{CompiledModules, CrateInfo};
use redox_data_structures::fx::FxIndexMap;
use redox_metadata::EncodedMetadata;
use redox_middle::dep_graph::{WorkProduct, WorkProductId};
use redox_middle::ty::TyCtxt;
use redox_session::Session;
use redox_session::config::OutputFilenames;

struct TheBackend;

impl CodegenBackend for TheBackend {
    fn name(&self) -> &'static str {
        "the-backend"
    }

    fn target_cpu(&self, _sess: &Session) -> String {
        "fake_target_cpu".to_owned()
    }

    fn codegen_crate(&self, _tcx: TyCtxt<'_>, _crate_info: &CrateInfo) -> Box<dyn Any> {
        Box::new(CompiledModules { modules: vec![], allocator_module: None })
    }

    fn join_codegen(
        &self,
        ongoing_codegen: Box<dyn Any>,
        _sess: &Session,
        _outputs: &OutputFilenames,
    ) -> (CompiledModules, FxIndexMap<WorkProductId, WorkProduct>) {
        let codegen_results = ongoing_codegen
            .downcast::<CompiledModules>()
            .expect("in join_codegen: ongoing_codegen is not a CompiledModules");
        (*codegen_results, FxIndexMap::default())
    }

    fn link(
        &self,
        sess: &Session,
        _compiled_modules: CompiledModules,
        crate_info: CrateInfo,
        _metadata: EncodedMetadata,
        outputs: &OutputFilenames,
    ) {
        use std::io::Write;

        use redox_session::config::{CrateType, OutFileName};
        use redox_session::output::out_filename;

        let crate_name = crate_info.local_crate_name;
        for &crate_type in sess.opts.crate_types.iter() {
            if crate_type != CrateType::Rlib {
                sess.dcx().fatal(format!("Crate type is {:?}", crate_type));
            }
            let output_name = out_filename(sess, crate_type, &outputs, crate_name);
            match output_name {
                OutFileName::Real(ref path) => {
                    let mut out_file = ::std::fs::File::create(path).unwrap();
                    writeln!(out_file, "This has been 'compiled' successfully.").unwrap();
                }
                OutFileName::Stdout => {
                    let mut stdout = std::io::stdout();
                    writeln!(stdout, "This has been 'compiled' successfully.").unwrap();
                }
            }
        }
    }
}

/// This is the entrypoint for a hot plugged redox_codegen_llvm
#[no_mangle]
pub fn __redox_codegen_backend() -> Box<dyn CodegenBackend> {
    Box::new(TheBackend)
}
