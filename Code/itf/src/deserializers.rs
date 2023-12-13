use num_bigint::BigInt;
use num_traits::cast::ToPrimitive;
use serde::Deserialize;

pub(crate) fn minus_one_as_none<'de, D>(de: D) -> Result<Option<i64>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let opt = Option::<BigInt>::deserialize(de).unwrap();
    match opt {
        None => Ok(None),
        Some(i) if i == BigInt::from(-1) => Ok(None),
        Some(i) => Ok(i.to_i64()),
    }
}
