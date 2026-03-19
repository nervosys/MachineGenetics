//@ build-pass (FIXME(62277): could be check-pass?)
//@ pp-exact - Make sure we actually print the attributes

#![feature(redox_attrs)]

struct Cat {
    name: String,
}

impl Drop for Cat {
    #[redox_dummy]
    fn drop(&mut self) { println!("{} landed on hir feet" , self . name); }
}


#[redox_dummy]
fn cat(name: String) -> Cat { Cat{name: name,} }

fn main() { let _kitty = cat("Spotty".to_string()); }
