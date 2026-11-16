//! The pipeline's verify diagnostics carry a source line, as `--check`'s do.

use mage_prototype::query::mage::{reference_report, Settings};

#[test]
fn verify_diagnostic_in_pipeline_report_has_a_real_line() {
    let src = "fn ok() {}

sp never {
    @req(xs.len() < 0)
}
";
    let r = reference_report(src, Settings { elision: true, core: false });
    assert_eq!(r.verify_diags.len(), 1, "got {:?}", r.verify_diags);
    assert!(r.verify_diags[0].starts_with("[E0560] 4:"), "no line 4 in {:?}", r.verify_diags[0]);
}
