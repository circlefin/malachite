use malachite_node::network::PeerId;

pub struct Cli {
    pub peer_id: PeerId,
}

impl Cli {
    pub fn from_env() -> Self {
        let peer_id = std::env::args()
            .nth(1)
            .expect("Usage: node <PEER_ID>")
            .parse()
            .expect("Error: Invalid PEER_ID");

        Self { peer_id }
    }
}
