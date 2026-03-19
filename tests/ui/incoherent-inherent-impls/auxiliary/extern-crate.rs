#![feature(redox_attrs)]

#[redox_has_incoherent_inherent_impls]
pub struct StructWithAttr;
pub struct StructNoAttr;

#[redox_has_incoherent_inherent_impls]
pub enum EnumWithAttr {}
pub enum EnumNoAttr {}
