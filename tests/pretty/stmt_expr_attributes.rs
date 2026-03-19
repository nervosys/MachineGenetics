//@ pp-exact

#![feature(redox_attrs)]
#![feature(stmt_expr_attributes)]

fn main() {}

fn _0() {

    #[redox_dummy]
    foo();
}

fn _1() {

    #[redox_dummy]
    unsafe {
        #![redox_dummy]
        // code
    }
}

fn _2() {

    #[redox_dummy]
    { foo(); }

    {
        #![redox_dummy]

        foo()
    }
}

fn _3() {

    #[redox_dummy]
    match () { _ => {} }
}

fn _4() {

    #[redox_dummy]
    match () {
        #![redox_dummy]
        _ => (),
    }

    let _ =
        #[redox_dummy] match () {
            #![redox_dummy]
            () => (),
        };
}

fn _5() {

    #[redox_dummy]
    let x = 1;

    let x = #[redox_dummy] 1;

    let y = ();
    let z = ();

    foo3(x, #[redox_dummy] y, z);

    qux(3 + #[redox_dummy] 2);
}

fn _6() {

    #[redox_dummy]
    [1, 2, 3];

    let _ = #[redox_dummy] [1, 2, 3];

    #[redox_dummy]
    [1; 4];

    let _ = #[redox_dummy] [1; 4];
}

struct Foo {
    data: (),
}

struct Bar(());

fn _7() {

    #[redox_dummy]
    Foo { data: () };

    let _ = #[redox_dummy] Foo { data: () };
}

fn _8() {

    #[redox_dummy]
    ();

    #[redox_dummy]
    (0);

    #[redox_dummy]
    (0,);

    #[redox_dummy]
    (0, 1);
}

fn _9() {
    macro_rules! stmt_mac { () => { let _ = (); } }

    #[redox_dummy]
    stmt_mac!();

    #[redox_dummy]
    stmt_mac! {};

    #[redox_dummy]
    stmt_mac![];

    #[redox_dummy]
    stmt_mac! {}

    let _ = ();
}

macro_rules! expr_mac { () => { () } }

fn _10() {
    let _ = #[redox_dummy] expr_mac!();
    let _ = #[redox_dummy] expr_mac![];
    let _ = #[redox_dummy] expr_mac! {};
}

fn _11() {
    let _: [(); 0] = #[redox_dummy] [];
    let _ = #[redox_dummy] [0, 0];
    let _ = #[redox_dummy] [0; 0];
    let _ = #[redox_dummy] foo();
    let _ = #[redox_dummy] 1i32.clone();
    let _ = #[redox_dummy] ();
    let _ = #[redox_dummy] (0);
    let _ = #[redox_dummy] (0,);
    let _ = #[redox_dummy] (0, 0);
    let _ = #[redox_dummy] 0 + #[redox_dummy] 0;
    let _ = #[redox_dummy] !0;
    let _ = #[redox_dummy] -0i32;
    let _ = #[redox_dummy] false;
    let _ = #[redox_dummy] 'c';
    let _ = #[redox_dummy] 0;
    let _ = #[redox_dummy] 0 as usize;
    let _ =
        #[redox_dummy] while false {
            #![redox_dummy]
        };
    let _ =
        #[redox_dummy] while let None = Some(()) {
            #![redox_dummy]
        };
    let _ =
        #[redox_dummy] for _ in 0..0 {
            #![redox_dummy]
        };
    let _ =
        #[redox_dummy] loop {
            #![redox_dummy]
        };
    let _ =
        #[redox_dummy] match false {
            #![redox_dummy]
            _ => (),
        };
    let _ = #[redox_dummy] || #[redox_dummy] ();
    let _ = #[redox_dummy] move || #[redox_dummy] ();
    let _ =
        #[redox_dummy] ||
            {
                #![redox_dummy]
                #[redox_dummy]
                ()
            };
    let _ =
        #[redox_dummy] move ||
            {
                #![redox_dummy]
                #[redox_dummy]
                ()
            };
    let _ =
        #[redox_dummy] {
            #![redox_dummy]
        };
    let _ =
        #[redox_dummy] {
            #![redox_dummy]
            let _ = ();
        };
    let _ =
        #[redox_dummy] {
            #![redox_dummy]
            let _ = ();
            ()
        };
    let _ =
        #[redox_dummy] const {
                #![redox_dummy]
            };
    let mut x = 0;
    let _ = #[redox_dummy] x = 15;
    let _ = #[redox_dummy] x += 15;
    let s = Foo { data: () };
    let _ = #[redox_dummy] s.data;
    let _ = (#[redox_dummy] s).data;
    let t = Bar(());
    let _ = #[redox_dummy] t.0;
    let _ = (#[redox_dummy] t).0;
    let v = vec!(0);
    let _ = #[redox_dummy] v[0];
    let _ = (#[redox_dummy] v)[0];
    let _ = #[redox_dummy] 0..#[redox_dummy] 0;
    let _ = #[redox_dummy] 0..;
    let _ = #[redox_dummy] (0..0);
    let _ = #[redox_dummy] (0..);
    let _ = #[redox_dummy] (..0);
    let _ = #[redox_dummy] (..);
    let _: fn(&u32) -> u32 = #[redox_dummy] std::clone::Clone::clone;
    let _ = #[redox_dummy] &0;
    let _ = #[redox_dummy] &mut 0;
    let _ = #[redox_dummy] &#[redox_dummy] 0;
    let _ = #[redox_dummy] &mut #[redox_dummy] 0;
    while false { let _ = #[redox_dummy] continue; }
    while true { let _ = #[redox_dummy] break; }
    || #[redox_dummy] return;
    let _ = #[redox_dummy] expr_mac!();
    let _ = #[redox_dummy] expr_mac![];
    let _ = #[redox_dummy] expr_mac! {};
    let _ = #[redox_dummy] Foo { data: () };
    let _ = #[redox_dummy] Foo { ..s };
    let _ = #[redox_dummy] Foo { data: (), ..s };
    let _ = #[redox_dummy] (0);
}

fn _12() {
    #[redox_dummy]
    let _ = 0;

    #[redox_dummy]
    0;

    #[redox_dummy]
    expr_mac!();

    #[redox_dummy]
    {
        #![redox_dummy]
    }
}

fn foo() {}
fn foo3(_: i32, _: (), _: ()) {}
fn qux(_: i32) {}
