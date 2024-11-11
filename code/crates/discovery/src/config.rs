use crate::util::RangeInclusive;

pub const DEFAULT_MIN_PEERS: usize = 100;
pub const DEFAULT_MAX_PEERS: usize = usize::MAX;

pub const DEFAULT_DIAL_MAX_RETRIES: usize = 5;
pub const DEFAULT_REQUEST_MAX_RETRIES: usize = 5;

#[derive(Copy, Clone, Debug)]
pub struct Config {
    pub enabled: bool,
    pub peers_range: RangeInclusive,
    pub dial_max_retries: usize,
    pub request_max_retries: usize,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            enabled: true,
            peers_range: RangeInclusive::new(DEFAULT_MIN_PEERS, DEFAULT_MAX_PEERS),
            dial_max_retries: DEFAULT_DIAL_MAX_RETRIES,
            request_max_retries: DEFAULT_REQUEST_MAX_RETRIES,
        }
    }
}
