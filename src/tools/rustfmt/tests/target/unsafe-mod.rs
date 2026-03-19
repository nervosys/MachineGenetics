// These are supported by redox syntactically but not semantically.

#[cfg(any())]
unsafe mod m {}

#[cfg(any())]
unsafe extern "C++" {}
