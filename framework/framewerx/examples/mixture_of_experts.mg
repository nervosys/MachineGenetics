// Sparse mixture-of-experts block. Router picks top-K experts per
// token. Each expert is a Linear; the router is a Linear over
// hidden -> num_experts.
//
// Dispatch note: a real MoE requires scatter-gather between router
// scores and expert outputs; this example demonstrates the SYNTAX
// for declaring the components. The simplified forward routes the
// normalized input directly through a single expert so the chain
// is shape-consistent for dispatch testing.

net MoEBlock {
    layer n1: LayerNorm(768);
    layer expert: Expert(768, 768);
    layer router: TopKRouter(768, 8);
    forward { expert(n1) }
}
