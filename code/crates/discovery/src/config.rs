use std::fmt;
use std::time::Duration;

const DEFAULT_NUM_OUTBOUND_PEERS: usize = 50;
const DEFAULT_NUM_INBOUND_PEERS: usize = 50;

const DEFAULT_MAX_CONNECTIONS_PER_PEER: usize = 5;

const DEFAULT_EPHEMERAL_CONNECTION_TIMEOUT: Duration = Duration::from_secs(15);

const DEFAULT_DIAL_MAX_RETRIES: usize = 5;
const DEFAULT_PEERS_REQUEST_MAX_RETRIES: usize = 5;
const DEFAULT_CONNECT_REQUEST_MAX_RETRIES: usize = 0;

const DEFAULT_MAX_PEERS_PER_RESPONSE: usize = 100;

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

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ConfigError {
    /// `num_inbound_peers` must be >= `num_outbound_peers`.
    InboundPeersBelowOutbound {
        num_outbound_peers: usize,
        num_inbound_peers: usize,
    },
    /// Kademlia selector requires the Kademlia bootstrap protocol.
    SelectorProtocolMismatch {
        selector: Selector,
        bootstrap_protocol: BootstrapProtocol,
    },
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InboundPeersBelowOutbound {
                num_outbound_peers,
                num_inbound_peers,
            } => write!(
                f,
                "number of inbound peers ({num_inbound_peers}) must be >= number of outbound peers ({num_outbound_peers})"
            ),
            Self::SelectorProtocolMismatch {
                selector,
                bootstrap_protocol,
            } => write!(
                f,
                "selector {selector:?} is only available with its matching bootstrap protocol, got {bootstrap_protocol:?}"
            ),
        }
    }
}

impl std::error::Error for ConfigError {}

#[derive(Copy, Clone, Debug)]
pub struct Config {
    pub enabled: bool,

    pub persistent_peers_only: bool,

    pub bootstrap_protocol: BootstrapProtocol,
    pub selector: Selector,

    pub num_outbound_peers: usize,
    pub num_inbound_peers: usize,

    pub max_connections_per_ip: usize,

    pub max_connections_per_peer: usize,

    pub ephemeral_connection_timeout: Duration,

    pub dial_max_retries: usize,
    pub request_max_retries: usize,
    pub connect_request_max_retries: usize,

    /// Maximum number of peer records to process or send per peers request/response.
    /// Limits the impact of a single response containing many records.
    pub max_peers_per_response: usize,
}

impl Default for Config {
    fn default() -> Self {
        if DEFAULT_NUM_INBOUND_PEERS < DEFAULT_NUM_OUTBOUND_PEERS {
            panic!("Number of inbound peers should be greater than or equal to number of outbound peers");
        }

        Self {
            enabled: true,

            persistent_peers_only: false,

            bootstrap_protocol: BootstrapProtocol::default(),
            selector: Selector::default(),

            num_outbound_peers: DEFAULT_NUM_OUTBOUND_PEERS,
            num_inbound_peers: DEFAULT_NUM_INBOUND_PEERS,

            max_connections_per_peer: DEFAULT_MAX_CONNECTIONS_PER_PEER,
            max_connections_per_ip: DEFAULT_NUM_INBOUND_PEERS,

            ephemeral_connection_timeout: DEFAULT_EPHEMERAL_CONNECTION_TIMEOUT,

            dial_max_retries: DEFAULT_DIAL_MAX_RETRIES,
            request_max_retries: DEFAULT_PEERS_REQUEST_MAX_RETRIES,
            connect_request_max_retries: DEFAULT_CONNECT_REQUEST_MAX_RETRIES,

            max_peers_per_response: DEFAULT_MAX_PEERS_PER_RESPONSE,
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

    /// Set the persistent_peers_only mode.
    ///
    /// # Arguments
    ///
    /// * `persistent_peers_only` - Whether to only allow connections from/to persistent peers.
    pub fn set_persistent_peers_only(&mut self, persistent_peers_only: bool) {
        self.persistent_peers_only = persistent_peers_only;
    }

    pub fn set_bootstrap_protocol(&mut self, protocol: BootstrapProtocol) {
        self.bootstrap_protocol = protocol;
    }

    pub fn set_selector(&mut self, selector: Selector) {
        self.selector = selector;
    }

    pub fn set_peers_bounds(
        &mut self,
        num_outbound_peers: usize,
        num_inbound_peers: usize,
    ) -> Result<(), ConfigError> {
        if num_inbound_peers < num_outbound_peers {
            return Err(ConfigError::InboundPeersBelowOutbound {
                num_outbound_peers,
                num_inbound_peers,
            });
        }

        self.num_outbound_peers = num_outbound_peers;
        self.num_inbound_peers = num_inbound_peers;
        Ok(())
    }

    pub fn set_max_connections_per_peer(&mut self, max_connections: usize) {
        self.max_connections_per_peer = max_connections;
    }

    pub fn set_ephemeral_connection_timeout(&mut self, timeout: Duration) {
        self.ephemeral_connection_timeout = timeout;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_caps_peers_per_response() {
        let config = Config::default();
        assert_eq!(config.max_peers_per_response, 100);
    }

    #[test]
    fn config_new_inherits_max_peers_per_response_default() {
        let config = Config::new(true);
        assert_eq!(config.max_peers_per_response, 100);
    }

    #[test]
    fn config_allows_custom_max_peers_per_response() {
        let config = Config {
            max_peers_per_response: 50,
            ..Default::default()
        };
        assert_eq!(config.max_peers_per_response, 50);
    }
}
