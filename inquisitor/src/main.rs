use clap::{Parser as _, ValueEnum};
use inquisitor_core::time::parse_duration;
use inquisitor_core::{Config, Method, MAX_CONNS};
use std::time::Duration;

#[derive(Debug, Copy, Clone, PartialEq, Eq, ValueEnum)]
enum CliMethod {
    Get,
    Post,
}

impl From<CliMethod> for Method {
    fn from(method: CliMethod) -> Self {
        match method {
            CliMethod::Get => Method::Get,
            CliMethod::Post => Method::Post,
        }
    }
}

#[derive(clap::Parser)]
#[command(about, version, disable_colored_help = true)]
struct Cli {
    /// Target URL for the load test
    #[clap(value_parser)]
    url: String,
    /// Number of requests to be sent
    ///
    /// If this and `--duration` (`-d`) are specified, the tests will end when
    /// the first of them is reached. If none is specified, a duration of 20
    /// seconds is used.
    #[clap(long, short = 'n', value_parser)]
    iterations: Option<usize>,
    /// Maximum number of HTTP connections to be kept opened concurrently
    #[clap(long, short = 'c', default_value_t = MAX_CONNS, value_parser)]
    connections: usize,
    /// Print the result of successful responses
    #[clap(long, action)]
    print_response: bool,
    /// If the response matches the string specified in this parameter, the
    /// response will be considered to be a failure
    #[clap(long, value_parser)]
    failed_body: Option<String>,
    /// Do not validate (TLS) certificates
    #[clap(long, short = 'k', action)]
    insecure: bool,
    /// HTTP method to use in the requests
    #[clap(long, default_value_t = CliMethod::Get, value_enum)]
    method: CliMethod,
    /// Body of the HTTP request (only used if method is POST)
    #[clap(long, short = 'b', value_parser)]
    request_body: Option<String>,
    /// Header entry for the HTTP request.
    ///
    /// The value should be in a KEY:VALUE format. Multiple key-value pairs can
    /// be passed, e.g.: `-H Content-Type:application/json -H SomeKey:SomeValue
    #[clap(long, short = 'H', value_parser)]
    header: Vec<String>,
    /// Do not print errors
    #[clap(long, action)]
    hide_errors: bool,
    /// Duration of the test.
    ///
    /// Should be a number (integer or decimal) followed by a "s", "m", or "h",
    /// for seconds, minutes and hours, respectively, without spaces. For
    /// example: "10s" (10 seconds), "1.5m" (1.5 minutes), "20h" (20 hours).
    ///
    /// If this and `--iterations` (`-n`) are specified, the tests will end when
    /// the first of them is reached. If none is specified, a duration of 20
    /// seconds is used.
    #[clap(long, short = 'd', value_parser = parse_duration)]
    duration: Option<Duration>,
    /// Path to a root CA certificate in PEM format, to be added to the request
    /// client's list of trusted CA certificates.
    #[clap(long, value_parser)]
    ca_cert: Option<String>,
}

impl From<Cli> for Config {
    fn from(cli: Cli) -> Self {
        Self {
            ca_cert: cli.ca_cert,
            connections: cli.connections,
            duration: cli.duration,
            failed_body: cli.failed_body,
            header: cli.header,
            hide_errors: cli.hide_errors,
            insecure: cli.insecure,
            iterations: cli.iterations,
            method: cli.method.into(),
            print_response: cli.print_response,
            request_body: cli.request_body,
            url: cli.url,
        }
    }
}

fn main() {
    let config = Cli::parse();
    inquisitor_core::run(config);
}
