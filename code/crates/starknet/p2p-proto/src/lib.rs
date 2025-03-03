#![allow(clippy::large_enum_variant)]

include!(concat!(env!("OUT_DIR"), "/p2p.rs"));

pub mod sync {
    include!(concat!(env!("OUT_DIR"), "/sync.rs"));
}

// pub mod certificate {
//     include!(concat!(env!("OUT_DIR"), "/certificate.rs"));
// }

impl From<Uint128> for u128 {
    fn from(value: Uint128) -> Self {
        (value.low as u128) | ((value.high as u128) << 64)
    }
}

impl From<u128> for Uint128 {
    fn from(value: u128) -> Self {
        Self {
            low: value as u64,
            high: (value >> 64) as u64,
        }
    }
}
