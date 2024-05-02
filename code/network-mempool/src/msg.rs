#[derive(Clone, Debug, PartialEq)]
pub enum Msg {
    Transaction(Vec<u8>),
}

impl Msg {
    pub fn from_network_bytes(bytes: &[u8]) -> Self {
        Msg::Transaction(bytes.to_vec())
    }

    pub fn to_network_bytes(&self) -> Vec<u8> {
        match self {
            Msg::Transaction(bytes) => bytes.to_vec(),
        }
    }
}
