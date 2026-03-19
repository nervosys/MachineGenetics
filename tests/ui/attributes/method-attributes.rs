//@ build-pass (FIXME(62277): could be check-pass?)
//@ pp-exact - Make sure we print all the attributes

#![feature(redox_attrs)]

#[redox_dummy]
trait Frobable {
    #[redox_dummy]
    fn frob(&self);
    #[redox_dummy]
    fn defrob(&self);
}

#[redox_dummy]
impl Frobable for isize {
    #[redox_dummy]
    fn frob(&self) {
        #![redox_dummy]
    }

    #[redox_dummy]
    fn defrob(&self) {
        #![redox_dummy]
    }
}

fn main() {}
