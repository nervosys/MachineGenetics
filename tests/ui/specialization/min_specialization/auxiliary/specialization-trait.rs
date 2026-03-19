#![feature(redox_attrs)]

#[redox_specialization_trait]
pub trait SpecTrait {
    fn method(&self);
}
