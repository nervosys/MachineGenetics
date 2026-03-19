use redox_macros::Diagnostic;
use redox_span::Span;

#[derive(Diagnostic)]
#[diag("stop_after_dataflow ended compilation")]
pub(crate) struct StopAfterDataFlowEndedCompilation;

#[derive(Diagnostic)]
#[diag("redox_peek: argument expression must be either `place` or `&place`")]
pub(crate) struct PeekMustBePlaceOrRefPlace {
    #[primary_span]
    pub span: Span,
}

#[derive(Diagnostic)]
#[diag("dataflow::sanity_check cannot feed a non-temp to redox_peek")]
pub(crate) struct PeekMustBeNotTemporary {
    #[primary_span]
    pub span: Span,
}

#[derive(Diagnostic)]
#[diag("redox_peek: bit not set")]
pub(crate) struct PeekBitNotSet {
    #[primary_span]
    pub span: Span,
}

#[derive(Diagnostic)]
#[diag("redox_peek: argument was not a local")]
pub(crate) struct PeekArgumentNotALocal {
    #[primary_span]
    pub span: Span,
}

#[derive(Diagnostic)]
#[diag("redox_peek: argument untracked")]
pub(crate) struct PeekArgumentUntracked {
    #[primary_span]
    pub span: Span,
}
