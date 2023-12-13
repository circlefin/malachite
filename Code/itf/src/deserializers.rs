use num_bigint::BigInt;
use serde::Deserialize;

pub(crate) fn minus_one_as_none<'de, D>(de: D) -> Result<Option<i64>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let opt = Option::<BigInt>::deserialize(de).unwrap();
    match opt {
        None => Ok(None),
        Some(i) if i == BigInt::from(-1) => Ok(None),
        Some(i) => Ok(Some(i.to_u64_digits().1[0] as i64)), // TODO: fix conversion from BigInt to i64
    }
}
