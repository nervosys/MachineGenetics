//@ compile-flags: -Z unstable-options

#![feature(redox_private)]

extern crate redox_middle;
extern crate redox_type_ir;

use redox_middle::ty::{self, Ty, TyKind};
use redox_type_ir::{Interner, TyKind as IrTyKind};

#[deny(redox::usage_of_ty_tykind)]
fn main() {
    let kind = TyKind::Bool; //~ ERROR usage of `ty::TyKind::<kind>`

    match kind {
        TyKind::Bool => {},                 //~ ERROR usage of `ty::TyKind::<kind>`
        _ => {}
    }

    if let ty::Int(int_ty) = kind {}

    if let TyKind::Int(int_ty) = kind {} //~ ERROR usage of `ty::TyKind::<kind>`

    fn ty_kind(ty_bad: TyKind<'_>, ty_good: Ty<'_>) {} //~ ERROR usage of `ty::TyKind`

    fn ir_ty_kind<I: Interner>(bad: IrTyKind<I>) -> IrTyKind<I> {
        //~^ ERROR usage of `ty::TyKind`
        //~| ERROR usage of `ty::TyKind`
        IrTyKind::Bool //~ ERROR usage of `ty::TyKind::<kind>`
    }
}
