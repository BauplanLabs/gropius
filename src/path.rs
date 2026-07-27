pub(crate) mod de;

#[cfg(any(feature = "client-reqwest", feature = "client-ureq"))]
pub(crate) mod ser;
