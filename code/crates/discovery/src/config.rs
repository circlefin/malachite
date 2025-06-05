use std::time::Duration;

const DEFAULT_NUM_OUTBOUND_PEERS: usize = 20;
const DEFAULT_NUM_INBOUND_PEERS: usize = 20;

const DEFAULT_MAX_CONNECTIONS_PER_PEER: usize = 5;

const DEFAULT_EPHEMERAL_CONNECTION_TIMEOUT: Duration = Duration::from_secs(15);

const DEFAULT_DIAL_MAX_RETRIES: usize = 5;
const DEFAULT_PEERS_REQUEST_MAX_RETRIES: usize = 5;
const DEFAULT_CONNECT_REQUEST_MAX_RETRIES: usize = 0;

#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub enum BootstrapProtocol {
    #[default]
    Kademlia,
    Full,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub enum Selector {
    #[default]
    Kademlia,
    Random,
}

#[derive(Copy, Clone, Debug)]
pub struct Config {
    pub enabled: bool,

    pub bootstrap_protocol: BootstrapProtocol,
    pub selector: Selector,

    pub num_outbound_peers: usize,
    pub num_inbound_peers: usize,

    pub max_connections_per_peer: usize,

    pub ephemeral_connection_timeout: Duration,

    pub dial_max_retries: usize,
    pub request_max_retries: usize,
    pub connect_request_max_retries: usize,
}

impl Default for Config {
    fn default() -> Self {
        if DEFAULT_NUM_INBOUND_PEERS < DEFAULT_NUM_OUTBOUND_PEERS {
            panic!("Number of inbound peers should be greater than or equal to number of outbound peers");
        }

        Self {
            enabled: true,

            bootstrap_protocol: BootstrapProtocol::default(),
            selector: Selector::default(),

            num_outbound_peers: DEFAULT_NUM_OUTBOUND_PEERS,
            num_inbound_peers: DEFAULT_NUM_INBOUND_PEERS,

            max_connections_per_peer: DEFAULT_MAX_CONNECTIONS_PER_PEER,

            ephemeral_connection_timeout: DEFAULT_EPHEMERAL_CONNECTION_TIMEOUT,

            dial_max_retries: DEFAULT_DIAL_MAX_RETRIES,
            request_max_retries: DEFAULT_PEERS_REQUEST_MAX_RETRIES,
            connect_request_max_retries: DEFAULT_CONNECT_REQUEST_MAX_RETRIES,
        }
    }
}

impl Config {
    pub fn new(enabled: bool) -> Self {
        Self {
            enabled,
            ..Default::default()
        }
    }

    pub fn set_bootstrap_protocol(&mut self, protocol: BootstrapProtocol) {
        self.bootstrap_protocol = protocol;
    }

    pub fn set_selector(&mut self, selector: Selector) {
        self.selector = selector;
    }

    pub fn set_peers_bounds(&mut self, num_outbound_peers: usize, num_inbound_peers: usize) {
        if num_inbound_peers < num_outbound_peers {
            panic!("Number of inbound peers should be greater than or equal to number of outbound peers");
        }

        self.num_outbound_peers = num_outbound_peers;
        self.num_inbound_peers = num_inbound_peers;
    }

    pub fn set_max_connections_per_peer(&mut self, max_connections: usize) {
        self.max_connections_per_peer = max_connections;
    }

    pub fn set_ephemeral_connection_timeout(&mut self, timeout: Duration) {
        self.ephemeral_connection_timeout = timeout;
    }
}
