pub const DEFAULT_DIAL_MAX_RETRIES: usize = 5;
pub const DEFAULT_REQUEST_MAX_RETRIES: usize = 5;

#[derive(Copy, Clone, Debug)]
pub struct Config {
    pub enabled: bool,
    pub dial_max_retries: usize,
    pub request_max_retries: usize,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            enabled: true,
            dial_max_retries: DEFAULT_DIAL_MAX_RETRIES,
            request_max_retries: DEFAULT_REQUEST_MAX_RETRIES,
        }
    }
}
