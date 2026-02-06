use serde::de;

/// Deserializes a boolean value from either a native boolean or a string
pub fn bool_from_anything<'de, D>(deserializer: D) -> Result<bool, D::Error>
where
    D: serde::Deserializer<'de>,
{
    struct BoolVisitor;

    impl<'de> de::Visitor<'de> for BoolVisitor {
        type Value = bool;

        fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
            write!(formatter, "a boolean or a string representing a boolean")
        }

        fn visit_bool<E>(self, v: bool) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            Ok(v)
        }

        fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            match v {
                "true" => Ok(true),
                "false" => Ok(false),
                other => Err(E::custom(format!("invalid boolean string: {other}"))),
            }
        }
    }

    deserializer.deserialize_any(BoolVisitor)
}

/// Deserializes a usize value from either a native integer or a string
pub fn usize_from_anything<'de, D>(deserializer: D) -> Result<usize, D::Error>
where
    D: serde::Deserializer<'de>,
{
    struct UsizeVisitor;

    impl<'de> de::Visitor<'de> for UsizeVisitor {
        type Value = usize;

        fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
            write!(formatter, "a usize or a string representing a usize")
        }

        fn visit_u64<E>(self, v: u64) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            usize::try_from(v)
                .map_err(|_| E::custom(format!("u64 value {} out of range for usize", v)))
        }

        fn visit_i64<E>(self, v: i64) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            usize::try_from(v)
                .map_err(|_| E::custom(format!("i64 value {} out of range for usize", v)))
        }

        fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            v.parse::<usize>()
                .map_err(|_| E::custom(format!("invalid usize string: {}", v)))
        }
    }

    deserializer.deserialize_any(UsizeVisitor)
}
