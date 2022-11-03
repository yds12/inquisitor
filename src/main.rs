use clap::{AppSettings, Parser as _, ValueEnum};
use hdrhistogram::Histogram;
use reqwest::ClientBuilder;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use tokio::sync::{Mutex, MutexGuard};

const ITERATIONS: usize = 1000;
const MAX_CONNS: usize = 12;

#[derive(Debug, Copy, Clone, PartialEq, ValueEnum)]
pub enum Method {
    GET,
    POST,
}

#[derive(clap::Parser)]
#[clap(about, setting = AppSettings::DisableColoredHelp)]
pub struct Config {
    /// Target URL for the load test
    #[clap(value_parser)]
    url: String,
    /// Number of requests to be sent
    #[clap(long, short = 'n', default_value_t = ITERATIONS, value_parser)]
    iterations: usize,
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
    #[clap(long, action)]
    ignore_certs: bool,
    /// HTTP method to use in the requests
    #[clap(long, default_value_t = Method::GET, value_enum)]
    method: Method,
    /// Body of the HTTP request (only used if method is POST)
    #[clap(long, short = 'b', value_parser)]
    request_body: Option<String>,
}

fn main() {
    let config = Config::parse();

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
        .and_then(|regex| Some(regex::Regex::new(&regex).expect("Failed to parse regex")));

    let request_body = Box::leak(Box::new(config.request_body)) as &Option<_>;

    let mut handles = Vec::new();

    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_io()
        .enable_time()
        .build()
        .unwrap();

    for _ in 0..config.connections {
        let client = ClientBuilder::new()
            .danger_accept_invalid_certs(config.ignore_certs)
            .build()
            .unwrap();

        let passes = passes.clone();
        let errors = errors.clone();
        let url = config.url.clone();
        let failed_regex = failed_regex.clone();
        let times = times.clone();

        let task = rt.spawn(async move {
            let mut total = passes.load(Ordering::Relaxed) + errors.load(Ordering::Relaxed);

            while total < config.iterations {
                let req_start_time = std::time::SystemTime::now();
                let response = match (config.method, request_body.as_deref()) {
                    (Method::GET, _) => client.get(&url).send().await,
                    (Method::POST, Some(body)) => client.post(&url).body(body).send().await,
                    (Method::POST, None) => client.post(&url).send().await,
                };
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
                            eprintln!("Response is 200 but body indicates an error: {}", body);
                            errors.fetch_add(1, Ordering::SeqCst);
                        } else {
                            passes.fetch_add(1, Ordering::SeqCst);

                            if config.print_response {
                                eprintln!("Response successful. Contents: {}", body);
                            }
                        }
                    }
                    Ok(res) if !res.status().is_success() => {
                        eprintln!("Response is not 200. Status code: {}", res.status());
                        errors.fetch_add(1, Ordering::SeqCst);
                    }
                    Err(e) => {
                        eprintln!("Request failed: {}", e);
                        errors.fetch_add(1, Ordering::SeqCst);
                    }
                    _ => unreachable!(),
                };

                total = passes.load(Ordering::Relaxed) + errors.load(Ordering::Relaxed);
            }
        });

        handles.push(task);
    }

    let times = rt.block_on(async {
        futures::future::join_all(handles).await;
        Arc::try_unwrap(times).expect("bug: could not unwrap Arc").into_inner()
    });

    let elapsed_ms = test_start_time.elapsed().unwrap().as_millis() as f64;
    print_results(config.iterations, times, elapsed_ms,
    errors.load(Ordering::Relaxed), passes.load(Ordering::Relaxed));
}

fn print_results(iterations: usize, times: Histogram<u64>, elapsed_ms: f64,
                 errors: usize, passes: usize) {
    let rps = (iterations as f64 / (elapsed_ms / 1_000.0)) as usize;

    println!(
        "total time: {:.3} s\nerrors: {:?}/{:?}\nthroughput: {} req./s",
        elapsed_ms / 1_000.0,
        errors,
        passes + errors,
        rps,
    );

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
