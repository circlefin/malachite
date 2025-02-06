use crate::streaming::Message;
use serde::Deserialize;
// Quint specification has its own Option type that is treated as enum in rust
// so message has to be extracted from it and be converted to rust's Option type
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(tag = "tag", content = "value")]
enum MessageOption {
    Some(Message),
    None,
}

pub(crate) fn quint_option_message<'de, D>(de: D) -> Result<Option<Message>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let opt = MessageOption::deserialize(de)?;
    match opt {
        MessageOption::Some(message) => Ok(Some(message)),
        MessageOption::None => Ok(None),
    }
}
