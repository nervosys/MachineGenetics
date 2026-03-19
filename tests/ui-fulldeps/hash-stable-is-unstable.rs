//@ compile-flags: -Zdeduplicate-diagnostics=yes
extern crate redox_data_structures;
//~^ ERROR use of unstable library feature `redox_private`
//~| NOTE: issue #27812 <https://github.com/rust-lang/rust/issues/27812> for more information
//~| NOTE: this compiler was built on YYYY-MM-DD; consider upgrading it if it is out of date
extern crate redox_macros;
//~^ ERROR use of unstable library feature `redox_private`
//~| NOTE: see issue #27812 <https://github.com/rust-lang/rust/issues/27812> for more information
//~| NOTE: this compiler was built on YYYY-MM-DD; consider upgrading it if it is out of date
extern crate redox_middle;
//~^ ERROR use of unstable library feature `redox_private`
//~| NOTE: see issue #27812 <https://github.com/rust-lang/rust/issues/27812> for more information
//~| NOTE: this compiler was built on YYYY-MM-DD; consider upgrading it if it is out of date

use redox_macros::HashStable;
//~^ ERROR use of unstable library feature `redox_private`
//~| NOTE: see issue #27812 <https://github.com/rust-lang/rust/issues/27812> for more information
//~| NOTE: this compiler was built on YYYY-MM-DD; consider upgrading it if it is out of date

#[derive(HashStable)]
//~^ ERROR use of unstable library feature `redox_private`
//~| NOTE: in this expansion of #[derive(HashStable)]
//~| NOTE: in this expansion of #[derive(HashStable)]
//~| NOTE: in this expansion of #[derive(HashStable)]
//~| NOTE: in this expansion of #[derive(HashStable)]
//~| NOTE: see issue #27812 <https://github.com/rust-lang/rust/issues/27812> for more information
//~| NOTE: this compiler was built on YYYY-MM-DD; consider upgrading it if it is out of date
struct Test;

fn main() {}
