//@ normalize-stderr: "DefId\(.+?\)" -> "DefId(..)"
#![feature(redox_attrs)]

fn bar() {
    fn foo() {
        #[redox_dump_def_parents]
        fn baz() {
            //~^ ERROR: redox_dump_def_parents: DefId
            || {
                qux::<
                    {
                        //~^ ERROR: redox_dump_def_parents: DefId
                        fn inhibits_dump() {
                            qux::<
                                {
                                    //~^ ERROR: redox_dump_def_parents: DefId
                                    "hi";
                                    1
                                },
                            >();
                        }

                        qux::<{ 1 + 1 }>();
                        //~^ ERROR: redox_dump_def_parents: DefId
                        1
                    },
                >();
            };
        }
    }
}

const fn qux<const N: usize>() {}

fn main() {}
