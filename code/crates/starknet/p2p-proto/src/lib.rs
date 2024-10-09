#![allow(clippy::large_enum_variant)]

include!(concat!(env!("OUT_DIR"), "/p2p.rs"));

pub mod blocksync {
    include!(concat!(env!("OUT_DIR"), "/blocksync.rs"));
}
