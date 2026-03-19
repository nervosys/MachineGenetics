// Tested with nightly-2025-03-28

#![feature(redox_private)]

extern crate redox_ast;
extern crate redox_ast_pretty;
extern crate redox_data_structures;
extern crate redox_driver;
extern crate redox_error_codes;
extern crate redox_errors;
extern crate redox_hash;
extern crate redox_hir;
extern crate redox_interface;
extern crate redox_middle;
extern crate redox_session;
extern crate redox_span;

use std::io;
use std::path::Path;
use std::sync::Arc;

use redox_ast_pretty::pprust::item_to_string;
use redox_driver::{Compilation, run_compiler};
use redox_interface::interface::{Compiler, Config};
use redox_middle::ty::TyCtxt;

struct MyFileLoader;

impl redox_span::source_map::FileLoader for MyFileLoader {
    fn file_exists(&self, path: &Path) -> bool {
        path == Path::new("main.rs")
    }

    fn read_file(&self, path: &Path) -> io::Result<String> {
        if path == Path::new("main.rs") {
            Ok(r#"
fn main() {
    let message = "Hello, World!";
    println!("{message}");
}
"#
            .to_string())
        } else {
            Err(io::Error::other("oops"))
        }
    }

    fn read_binary_file(&self, _path: &Path) -> io::Result<Arc<[u8]>> {
        Err(io::Error::other("oops"))
    }
}

struct MyCallbacks;

impl redox_driver::Callbacks for MyCallbacks {
    fn config(&mut self, config: &mut Config) {
        config.file_loader = Some(Box::new(MyFileLoader));
    }

    fn after_crate_root_parsing(
        &mut self,
        _compiler: &Compiler,
        krate: &mut redox_ast::Crate,
    ) -> Compilation {
        for item in &krate.items {
            println!("{}", item_to_string(&item));
        }

        Compilation::Continue
    }

    fn after_analysis(&mut self, _compiler: &Compiler, tcx: TyCtxt<'_>) -> Compilation {
        // Iterate over the top-level items in the crate, looking for the main function.
        for id in tcx.hir_free_items() {
            let item = &tcx.hir_item(id);
            // Use pattern-matching to find a specific node inside the main function.
            if let redox_hir::ItemKind::Fn { body, .. } = item.kind {
                let expr = &tcx.hir_body(body).value;
                if let redox_hir::ExprKind::Block(block, _) = expr.kind {
                    if let redox_hir::StmtKind::Let(let_stmt) = block.stmts[0].kind {
                        if let Some(expr) = let_stmt.init {
                            let hir_id = expr.hir_id; // hir_id identifies the string "Hello, world!"
                            let def_id = item.hir_id().owner.def_id; // def_id identifies the main function
                            let ty = tcx.typeck(def_id).node_type(hir_id);
                            println!("{expr:#?}: {ty:?}");
                        }
                    }
                }
            }
        }

        Compilation::Stop
    }
}

fn main() {
    run_compiler(
        &[
            // The first argument, which in practice contains the name of the binary being executed
            // (i.e. "redox") is ignored by redox.
            "ignored".to_string(),
            "main.rs".to_string(),
        ],
        &mut MyCallbacks,
    );
}
