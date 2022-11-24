use std::time::Duration;

/// Default run duration
pub const DEFAULT_DURATION_SECS: u64 = 20;

/// HTTP method
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum Method {
    Get,
    Post,
}

impl Default for Method {
    fn default() -> Self {
        Self::Get
    }
}

/// Configuration of the load test runner
#[derive(Default)]
pub struct Config {
    /// Target URL for the load test
    pub url: String,
    /// Number of requests to be sent
    ///
    /// If this and `--duration` (`-d`) are specified, the tests will end when
    /// the first of them is reached. If none is specified, a duration of 20
    /// seconds is used.
    pub iterations: Option<usize>,
    /// Maximum number of HTTP connections to be kept opened concurrently
    pub connections: usize,
    /// Print the result of successful responses
    pub print_response: bool,
    /// If the response matches the string specified in this parameter, the
    /// response will be considered to be a failure
    pub failed_body: Option<String>,
    /// Do not validate (TLS) certificates
    pub insecure: bool,
    /// HTTP method to use in the requests
    pub method: Method,
    /// Body of the HTTP request (only used if method is POST)
    pub request_body: Option<String>,
    /// Header entry for the HTTP request.
    ///
    /// The value should be in a KEY:VALUE format. Multiple key-value pairs can
    /// be passed, e.g.: `-H Content-Type:application/json -H SomeKey:SomeValue
    pub header: Vec<String>,
    /// Do not print errors
    pub hide_errors: bool,
    /// Duration of the test.
    ///
    /// Should be a number (integer or decimal) followed by a "s", "m", or "h",
    /// for seconds, minutes and hours, respectively, without spaces. For
    /// example: "10s" (10 seconds), "1.5m" (1.5 minutes), "20h" (20 hours).
    ///
    /// If this and `--iterations` (`-n`) are specified, the tests will end when
    /// the first of them is reached. If none is specified, a duration of 20
    /// seconds is used.
    pub duration: Option<Duration>,
    /// Path to a root CA certificate in PEM format, to be added to the request
    /// client's list of trusted CA certificates.
    pub ca_cert: Option<String>,
}

impl Config {
    /// Get the effective maximum number of iterations and duration (in
    /// microseconds), as a function of the configurations set by the user
    pub fn iterations_and_duration(&self) -> (usize, u64) {
        match (self.iterations, self.duration) {
            (None, None) => (usize::MAX, DEFAULT_DURATION_SECS * 1_000_000),
            (Some(i), None) => (i, u64::MAX),
            (None, Some(d)) => (usize::MAX, d.as_micros() as u64),
            (Some(i), Some(d)) => (i, d.as_micros() as u64),
        }
    }
}
