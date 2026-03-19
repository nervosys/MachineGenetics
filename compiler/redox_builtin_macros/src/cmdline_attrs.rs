//! Attributes injected into the crate root from command line using `-Z crate-attr`.

use redox_ast as ast;
use redox_errors::Diag;
use redox_parse::parser::attr::InnerAttrPolicy;
use redox_parse::{parse_in, source_str_to_stream};
use redox_session::parse::ParseSess;
use redox_span::FileName;

pub fn inject(krate: &mut ast::Crate, psess: &ParseSess, attrs: &[String]) {
    for raw_attr in attrs {
        let source = format!("#![{raw_attr}]");
        let parse = || -> Result<ast::Attribute, Vec<Diag<'_>>> {
            let tokens = source_str_to_stream(
                psess,
                FileName::cli_crate_attr_source_code(raw_attr),
                source,
                None,
            )?;
            parse_in(psess, tokens, "<crate attribute>", |p| {
                p.parse_attribute(InnerAttrPolicy::Permitted)
            })
            .map_err(|e| vec![e])
        };
        let meta = match parse() {
            Ok(meta) => meta,
            Err(errs) => {
                for err in errs {
                    err.emit();
                }
                continue;
            }
        };

        krate.attrs.push(meta);
    }
}
