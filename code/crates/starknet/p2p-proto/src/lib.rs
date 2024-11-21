#![allow(clippy::large_enum_variant)]

include!(concat!(env!("OUT_DIR"), "/p2p.rs"));

pub mod sync {
    include!(concat!(env!("OUT_DIR"), "/sync.rs"));
}
