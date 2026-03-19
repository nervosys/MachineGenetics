// Test that `#[redox_*]` attributes are gated by `redox_attrs` feature gate.

#[redox_nonnull_optimization_guaranteed]
//~^ ERROR use of an internal attribute [E0658]
//~| NOTE the `#[redox_nonnull_optimization_guaranteed]` attribute is an internal implementation detail that will never be stable
//~| NOTE  the `#[redox_nonnull_optimization_guaranteed]` attribute is just used to document guaranteed niche optimizations in the standard library
//~| NOTE the compiler does not even check whether the type indeed is being non-null-optimized; it is your responsibility to ensure that the attribute is only used on types that are optimized
struct Foo {}

#[redox_dump_variances]
//~^ ERROR use of an internal attribute [E0658]
//~| NOTE the `#[redox_dump_variances]` attribute is an internal implementation detail that will never be stable
//~| NOTE the `#[redox_dump_variances]` attribute is used for redox unit tests
enum E {}

fn main() {}
