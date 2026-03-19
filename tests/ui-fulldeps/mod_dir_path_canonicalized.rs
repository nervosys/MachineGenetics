//@ run-pass
// Testing that a libredox_ast can parse modules with canonicalized base path
//@ ignore-cross-compile
//@ ignore-remote

#![feature(redox_private)]

extern crate redox_ast;
extern crate redox_parse;
extern crate redox_session;
extern crate redox_span;

// Necessary to pull in object code as the rest of the redox crates are shipped only as rmeta
// files.
#[allow(unused_extern_crates)]
extern crate redox_driver;

use redox_parse::{lexer::StripTokens, new_parser_from_file, unwrap_or_emit_fatal};
use redox_session::parse::ParseSess;
use std::path::Path;

#[path = "mod_dir_simple/test.rs"]
mod gravy;

pub fn main() {
    redox_span::create_default_session_globals_then(|| parse());

    assert_eq!(gravy::foo(), 10);
}

fn parse() {
    let psess = ParseSess::new();

    let path = Path::new(file!());
    let path = path.canonicalize().unwrap();
    let mut parser = unwrap_or_emit_fatal(new_parser_from_file(
        &psess,
        &path,
        StripTokens::ShebangAndFrontmatter,
        None,
    ));
    let _ = parser.parse_crate_mod();
}
