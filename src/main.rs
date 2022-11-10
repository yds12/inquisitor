use clap::{Parser as _, ValueEnum};
use hdrhistogram::Histogram;
use reqwest::ClientBuilder;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;

const MAX_CONNS: usize = 12;
const DEFAULT_DURATION_SECS: u64 = 20;

#[derive(Debug, Copy, Clone, PartialEq, Eq, ValueEnum)]
pub enum Method {
    GET,
    POST,
}

#[derive(clap::Parser)]
#[command(about, disable_colored_help = true)]
pub struct Config {
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
    #[clap(long, default_value_t = Method::GET, value_enum)]
    method: Method,
    /// Body of the HTTP request (only used if method is POST)
    #[clap(long, short = 'b', value_parser)]
    request_body: Option<String>,
    /// Header entry for the HTTP request.
    ///
    /// The value should be in a KEY:VALUE format. Multiple key-value pairs can
    /// be passed, e.g.: `-h Content-Type:application/json -h SomeKey:SomeValue
    #[clap(long, short = 'h', value_parser)]
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
    /// If this and `--duration` (`-d`) are specified, the tests will end when
    /// the first of them is reached. If none is specified, a duration of 20
    /// seconds is used.
    #[clap(long, short = 'd', value_parser = parse_duration)]
    duration: Option<Duration>,
}

#[derive(Debug)]
enum InquisitorError {
    DurationParseError,
}

impl std::fmt::Display for InquisitorError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        match self {
            Self::DurationParseError => write!(f, ""),
        }
    }
}

impl std::error::Error for InquisitorError {}

fn main() {
    let should_exit = Arc::new(AtomicBool::new(false));
    let should_exit_clone = should_exit.clone();

    ctrlc::set_handler(move || {
        let previously_set = should_exit_clone.fetch_or(true, Ordering::SeqCst);

        if previously_set {
            std::process::exit(130);
        }
    })
    .expect("Error setting signal handler");

    let config = Config::parse();

    let (iterations, duration) = match (config.iterations, config.duration) {
        (None, None) => (usize::MAX, DEFAULT_DURATION_SECS * 1_000_000),
        (Some(i), None) => (i, u64::MAX),
        (None, Some(d)) => (usize::MAX, d.as_micros() as u64),
        (Some(i), Some(d)) => (i, d.as_micros() as u64),
    };

    let mut headers = HashMap::new();
    for header in config.header {
        if let Some((k, v)) = header.split_once(':') {
            headers.insert(k.to_string(), v.to_string());
        }
    }

    // histogram of response times, recorded in microseconds
    let times = Arc::new(Mutex::new(
        Histogram::<u64>::new_with_max(1_000_000_000_000, 3)
            .expect("Failed to create histogram for response times: invalid parameters"),
    ));

    let passes = Arc::new(AtomicUsize::new(0));
    let errors = Arc::new(AtomicUsize::new(0));

    let test_start_time = std::time::SystemTime::now();

    let failed_regex = config
        .failed_body
        .map(|regex| regex::Regex::new(&regex).expect("Failed to parse regex"));

    let request_body = Box::leak(Box::new(config.request_body)) as &Option<_>;

    let mut handles = Vec::new();

    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_io()
        .enable_time()
        .build()
        .unwrap();

    for _ in 0..config.connections {
        let client = ClientBuilder::new()
            .danger_accept_invalid_certs(config.insecure)
            .build()
            .unwrap();

        let passes = passes.clone();
        let errors = errors.clone();
        let url = config.url.clone();
        let headers = headers.clone();
        let failed_regex = failed_regex.clone();
        let times = times.clone();
        let should_exit = should_exit.clone();

        let task = rt.spawn(async move {
            let mut total = passes.load(Ordering::Relaxed) + errors.load(Ordering::Relaxed);
            let mut total_elapsed = test_start_time.elapsed().unwrap().as_micros() as u64;

            while total < iterations && total_elapsed < duration {
                if should_exit.load(Ordering::Relaxed) {
                    break;
                }

                let mut builder = match config.method {
                    Method::GET => client.get(&url),
                    Method::POST => client.post(&url),
                };

                if let Some(body) = request_body.as_deref() {
                    builder = builder.body(body);
                }

                for (k, v) in &headers {
                    builder = builder.header(k, v);
                }

                let req_start_time = std::time::SystemTime::now();
                let response = builder.send().await;
                let elapsed = req_start_time.elapsed().unwrap().as_micros() as u64;
                times
                    .lock()
                    .await
                    .record(elapsed)
                    .expect("time out of bounds");

                match response {
                    Ok(res) if res.status().is_success() && failed_regex.is_none() => {
                        passes.fetch_add(1, Ordering::SeqCst);
                        if config.print_response {
                            println!(
                                "Response successful. Content: {}",
                                res.text().await.unwrap()
                            );
                        }
                    }
                    Ok(res) if res.status().is_success() && failed_regex.is_some() => {
                        let body = res.text().await.unwrap();

                        if failed_regex.as_ref().unwrap().is_match(&body) {
                            if !config.hide_errors {
                                eprintln!("Response is 200 but body indicates an error: {}", body);
                            }
                            errors.fetch_add(1, Ordering::SeqCst);
                        } else {
                            passes.fetch_add(1, Ordering::SeqCst);

                            if config.print_response {
                                println!("Response successful. Contents: {}", body);
                            }
                        }
                    }
                    Ok(res) if !res.status().is_success() => {
                        if !config.hide_errors {
                            eprintln!("Response is not 200. Status code: {}", res.status());
                        }
                        errors.fetch_add(1, Ordering::SeqCst);
                    }
                    Err(e) => {
                        if !config.hide_errors {
                            eprintln!("Request failed: {}", e);
                        }
                        errors.fetch_add(1, Ordering::SeqCst);
                    }
                    _ => unreachable!(),
                };

                total = passes.load(Ordering::Relaxed) + errors.load(Ordering::Relaxed);
                total_elapsed = test_start_time.elapsed().unwrap().as_micros() as u64;
            }
        });

        handles.push(task);
    }

    let times = rt.block_on(async {
        futures::future::join_all(handles).await;
        Arc::try_unwrap(times)
            .expect("bug: could not unwrap Arc")
            .into_inner()
    });

    let elapsed_ms = test_start_time.elapsed().unwrap().as_millis() as f64;
    print_results(
        times,
        elapsed_ms,
        errors.load(Ordering::Relaxed),
        passes.load(Ordering::Relaxed),
    );
}

fn print_results(times: Histogram<u64>, elapsed_ms: f64, errors: usize, passes: usize) {
    let iterations = passes + errors;
    let rps = (iterations as f64 / (elapsed_ms / 1_000.0)) as usize;

    println!("total time: {:.3} s", elapsed_ms / 1_000.0,);
    print!("errors: {}/{}", errors, iterations,);

    if errors > 0 {
        println!(" ({:.2}%)", (errors as f64 / iterations as f64) * 100.0);
    } else {
        println!();
    }
    println!("throughput: {} req./s", rps,);

    println!(
        "response times:\n\tmean\t{:.3} ms\n\tst.dev\t{:.3} ms\n\tmin\t{:.3} ms\n\tmax\t{:.3} ms",
        times.mean() / 1000.0,
        times.stdev() / 1000.0,
        times.min() as f64 / 1000.0,
        times.max() as f64 / 1000.0,
    );

    println!(
        "latencies:\n\t50%\t{:.3} ms\n\t75%\t{:.3} ms\n\t90%\t{:.3} ms\n\t95%\t{:.3} ms\n\t99%\t{:.3} ms\n\t99.9%\t{:.3} ms",
        times.value_at_quantile(0.5) as f64 / 1000.0,
        times.value_at_quantile(0.75) as f64 / 1000.0,
        times.value_at_quantile(0.9) as f64 / 1000.0,
        times.value_at_quantile(0.95) as f64 / 1000.0,
        times.value_at_quantile(0.99) as f64 / 1000.0,
        times.value_at_quantile(0.999) as f64 / 1000.0,
    );
}

fn parse_duration(duration: &str) -> Result<Duration, InquisitorError> {
    let re = regex::Regex::new(r"(\d\d*(?:\.\d\d*)??)([smh])").expect("Bug: wrong regex");
    let cap = re
        .captures(duration)
        .ok_or(InquisitorError::DurationParseError)?;

    let base = cap[1]
        .parse::<f64>()
        .map_err(|_| InquisitorError::DurationParseError)?;
    let mul: f64 = match &cap[2] {
        "s" => 1_000_000.0,
        "m" => 60.0 * 1_000_000.0,
        "h" => 60.0 * 60.0 * 1_000_000.0,
        _ => unreachable!(),
    };

    Ok(Duration::from_micros((base * mul) as u64))
}
