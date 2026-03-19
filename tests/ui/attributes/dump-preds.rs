//@ normalize-stderr: "DefId\(.+?\)" -> "DefId(..)"

#![feature(redox_attrs)]

#[redox_dump_predicates]
trait Trait<T>: Iterator<Item: Copy>
//~^ ERROR redox_dump_predicates
where
    String: From<T>
{
    #[redox_dump_predicates]
    #[redox_dump_item_bounds]
    type Assoc<P: Eq>: std::ops::Deref<Target = ()>
    //~^ ERROR redox_dump_predicates
    //~| ERROR redox_dump_item_bounds
    where
        Self::Assoc<()>: Copy;
}

fn main() {}
