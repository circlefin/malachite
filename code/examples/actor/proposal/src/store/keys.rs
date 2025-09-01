use crate::types::{BlockHash, Height};
use core::mem::size_of;
use malachitebft_core_types::Round;

pub type UndecidedValueKey = (HeightKey, RoundKey, BlockHashKey);

#[derive(Copy, Clone, Debug)]
pub struct HeightKey;

impl redb::Value for HeightKey {
    type SelfType<'a> = Height;
    type AsBytes<'a> = [u8; size_of::<u64>()];

    fn fixed_width() -> Option<usize> {
        Some(size_of::<u64>())
    }

    fn from_bytes<'a>(data: &'a [u8]) -> Self::SelfType<'a>
    where
        Self: 'a,
    {
        let height = <u64 as redb::Value>::from_bytes(data);

        Height::new(height)
    }

    fn as_bytes<'a, 'b: 'a>(value: &'a Self::SelfType<'b>) -> Self::AsBytes<'a>
    where
        Self: 'a,
        Self: 'b,
    {
        <u64 as redb::Value>::as_bytes(&value.as_u64())
    }

    fn type_name() -> redb::TypeName {
        redb::TypeName::new("Height")
    }
}

impl redb::Key for HeightKey {
    fn compare(data1: &[u8], data2: &[u8]) -> std::cmp::Ordering {
        <u64 as redb::Key>::compare(data1, data2)
    }
}

#[derive(Copy, Clone, Debug)]
pub struct RoundKey;

impl redb::Value for RoundKey {
    type SelfType<'a> = Round;
    type AsBytes<'a> = [u8; size_of::<i64>()];

    fn fixed_width() -> Option<usize> {
        Some(size_of::<i64>())
    }

    fn from_bytes<'a>(data: &'a [u8]) -> Self::SelfType<'a>
    where
        Self: 'a,
    {
        let round = <i64 as redb::Value>::from_bytes(data);
        Round::from(round)
    }

    fn as_bytes<'a, 'b: 'a>(value: &'a Self::SelfType<'b>) -> Self::AsBytes<'a>
    where
        Self: 'a,
        Self: 'b,
    {
        <i64 as redb::Value>::as_bytes(&value.as_i64())
    }

    fn type_name() -> redb::TypeName {
        redb::TypeName::new("Round")
    }
}

impl redb::Key for RoundKey {
    fn compare(data1: &[u8], data2: &[u8]) -> std::cmp::Ordering {
        <i64 as redb::Key>::compare(data1, data2)
    }
}

#[derive(Copy, Clone, Debug)]
pub struct BlockHashKey;

impl redb::Value for BlockHashKey {
    type SelfType<'a> = BlockHash;
    type AsBytes<'a> = &'a [u8; 32];

    fn fixed_width() -> Option<usize> {
        Some(32)
    }

    fn from_bytes<'a>(data: &'a [u8]) -> Self::SelfType<'a>
    where
        Self: 'a,
    {
        let bytes = <[u8; 32] as redb::Value>::from_bytes(data);
        BlockHash::new(bytes)
    }

    fn as_bytes<'a, 'b: 'a>(value: &'a Self::SelfType<'b>) -> Self::AsBytes<'a>
    where
        Self: 'a,
        Self: 'b,
    {
        value.as_bytes()
    }

    fn type_name() -> redb::TypeName {
        redb::TypeName::new("BlockHash")
    }
}

impl redb::Key for BlockHashKey {
    fn compare(data1: &[u8], data2: &[u8]) -> std::cmp::Ordering {
        <[u8; 32] as redb::Key>::compare(data1, data2)
    }
}
