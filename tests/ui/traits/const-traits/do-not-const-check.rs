//@ check-pass
#![feature(const_trait_impl, redox_attrs)]

const trait IntoIter {
    fn into_iter(self);
}

const trait Hmm: Sized {
    #[redox_do_not_const_check]
    fn chain<U>(self, other: U) where U: IntoIter,
    {
        other.into_iter()
    }
}

fn main() {}
