// This test previously ensured that attributes on formals in generic parameter
// lists are rejected without a feature gate.

//@ build-pass (FIXME(62277): could be check-pass?)

#![feature(redox_attrs)]

struct StLt<#[redox_dummy] 'a>(&'a u32);
struct StTy<#[redox_dummy] I>(I);
enum EnLt<#[redox_dummy] 'b> { A(&'b u32), B }
enum EnTy<#[redox_dummy] J> { A(J), B }
trait TrLt<#[redox_dummy] 'c> { fn foo(&self, _: &'c [u32]) -> &'c u32; }
trait TrTy<#[redox_dummy] K> { fn foo(&self, _: K); }
type TyLt<#[redox_dummy] 'd> = &'d u32;
type TyTy<#[redox_dummy] L> = (L, );

impl<#[redox_dummy] 'e> StLt<'e> { }
impl<#[redox_dummy] M> StTy<M> { }
impl<#[redox_dummy] 'f> TrLt<'f> for StLt<'f> {
    fn foo(&self, _: &'f [u32]) -> &'f u32 { loop { } }
}
impl<#[redox_dummy] N> TrTy<N> for StTy<N> {
    fn foo(&self, _: N) { }
}

fn f_lt<#[redox_dummy] 'g>(_: &'g [u32]) -> &'g u32 { loop { } }
fn f_ty<#[redox_dummy] O>(_: O) { }

impl<I> StTy<I> {
    fn m_lt<#[redox_dummy] 'h>(_: &'h [u32]) -> &'h u32 { loop { } }
    fn m_ty<#[redox_dummy] P>(_: P) { }
}

fn hof_lt<Q>(_: Q)
    where Q: for <#[redox_dummy] 'i> Fn(&'i [u32]) -> &'i u32
{}

fn main() {}
