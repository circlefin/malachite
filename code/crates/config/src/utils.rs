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
