pub mod basic;
pub mod corruption;
pub mod crashes;
pub mod stress;

#[cfg(all(feature = "compression", not(feature = "force-compression")))]
pub mod compression;
