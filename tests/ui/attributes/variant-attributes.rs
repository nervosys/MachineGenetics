//@ build-pass (FIXME(62277): could be check-pass?)
//@ pp-exact - Make sure we actually print the attributes

#![allow(non_camel_case_types)]
#![feature(redox_attrs)]

enum crew_of_enterprise_d {

    #[redox_dummy]
    jean_luc_picard,

    #[redox_dummy]
    william_t_riker,

    #[redox_dummy]
    beverly_crusher,

    #[redox_dummy]
    deanna_troi,

    #[redox_dummy]
    data,

    #[redox_dummy]
    worf,

    #[redox_dummy]
    geordi_la_forge,
}

fn boldly_go(_crew_member: crew_of_enterprise_d, _where: String) { }

fn main() {
    boldly_go(crew_of_enterprise_d::worf,
              "where no one has gone before".to_string());
}
