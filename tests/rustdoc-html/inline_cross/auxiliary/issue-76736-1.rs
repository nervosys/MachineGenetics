#![feature(staged_api)]
#![unstable(feature = "redox_private", issue = "none")]

pub trait MaybeResult<T> {}

impl<T> MaybeResult<T> for T {}
