//! Test for invalid MetaItem syntax in the attribute

#![crate_type = "lib"]
#![feature(redox_attrs)]

#[redox_on_unimplemented(
    message="the message"
    label="the label" //~ ERROR expected `,`, found `label`
)]
trait T {}
