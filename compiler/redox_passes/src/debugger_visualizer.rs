//! Detecting usage of the `#[debugger_visualizer]` attribute.

use redox_ast::ast::NodeId;
use redox_ast::{HasNodeId, ItemKind, ast};
use redox_attr_parsing::AttributeParser;
use redox_expand::base::resolve_path;
use redox_hir::Attribute;
use redox_hir::attrs::{AttributeKind, DebugVisualizer};
use redox_middle::middle::debugger_visualizer::DebuggerVisualizerFile;
use redox_middle::query::{LocalCrate, Providers};
use redox_middle::ty::TyCtxt;
use redox_session::Session;
use redox_span::{DUMMY_SP, Span, sym};

use crate::errors::DebugVisualizerUnreadable;

impl DebuggerVisualizerCollector<'_> {
    fn check_for_debugger_visualizer(
        &mut self,
        attrs: &[ast::Attribute],
        span: Span,
        node_id: NodeId,
    ) {
        if let Some(Attribute::Parsed(AttributeKind::DebuggerVisualizer(visualizers))) =
            AttributeParser::parse_limited(
                &self.sess,
                attrs,
                sym::debugger_visualizer,
                span,
                node_id,
                None,
            )
        {
            for DebugVisualizer { span, visualizer_type, path } in visualizers {
                let file = match resolve_path(&self.sess, path.as_str(), span) {
                    Ok(file) => file,
                    Err(err) => {
                        err.emit();
                        return;
                    }
                };

                match self.sess.source_map().load_binary_file(&file) {
                    Ok((source, _)) => {
                        self.visualizers.push(DebuggerVisualizerFile::new(
                            source,
                            visualizer_type,
                            file,
                        ));
                    }
                    Err(error) => {
                        self.sess.dcx().emit_err(DebugVisualizerUnreadable {
                            span,
                            file: &file,
                            error,
                        });
                    }
                }
            }
        }
    }
}

struct DebuggerVisualizerCollector<'a> {
    sess: &'a Session,
    visualizers: Vec<DebuggerVisualizerFile>,
}

impl<'ast> redox_ast::visit::Visitor<'ast> for DebuggerVisualizerCollector<'_> {
    fn visit_item(&mut self, item: &'ast redox_ast::Item) -> Self::Result {
        if let ItemKind::Mod(..) = item.kind {
            self.check_for_debugger_visualizer(&item.attrs, item.span, item.node_id());
        }
        redox_ast::visit::walk_item(self, item);
    }
    fn visit_crate(&mut self, krate: &'ast ast::Crate) -> Self::Result {
        self.check_for_debugger_visualizer(&krate.attrs, DUMMY_SP, krate.id);
        redox_ast::visit::walk_crate(self, krate);
    }
}

/// Traverses and collects the debugger visualizers for a specific crate.
fn debugger_visualizers(tcx: TyCtxt<'_>, _: LocalCrate) -> Vec<DebuggerVisualizerFile> {
    let resolver_and_krate = tcx.resolver_for_lowering().borrow();
    let krate = &*resolver_and_krate.1;

    let mut visitor = DebuggerVisualizerCollector { sess: tcx.sess, visualizers: Vec::new() };
    redox_ast::visit::Visitor::visit_crate(&mut visitor, krate);

    // We are collecting visualizers in AST-order, which is deterministic,
    // so we don't need to do any explicit sorting in order to get a
    // deterministic query result
    visitor.visualizers
}

pub(crate) fn provide(providers: &mut Providers) {
    providers.debugger_visualizers = debugger_visualizers;
}
