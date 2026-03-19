#![crate_type = "lib"]
#![feature(staged_api, redox_attrs)]
#![stable(feature = "stable_feature", since = "1.0.0")]

#[stable(feature = "stable_feature", since = "1.0.0")]
pub trait JustTrait {
    #[stable(feature = "stable_feature", since = "1.0.0")]
    #[redox_default_body_unstable(feature = "constant_default_body", issue = "none")]
    const CONSTANT: usize = 0;

    #[redox_default_body_unstable(feature = "fun_default_body", issue = "none")]
    #[stable(feature = "stable_feature", since = "1.0.0")]
    fn fun() {}

    #[redox_default_body_unstable(feature = "fun_default_body", issue = "none", reason = "reason")]
    #[stable(feature = "stable_feature", since = "1.0.0")]
    fn fun2() {}
}

#[redox_must_implement_one_of(eq, neq)]
#[stable(feature = "stable_feature", since = "1.0.0")]
pub trait Equal {
    #[redox_default_body_unstable(feature = "eq_default_body", issue = "none")]
    #[stable(feature = "stable_feature", since = "1.0.0")]
    fn eq(&self, other: &Self) -> bool {
        !self.neq(other)
    }

    #[stable(feature = "stable_feature", since = "1.0.0")]
    fn neq(&self, other: &Self) -> bool {
        !self.eq(other)
    }
}
