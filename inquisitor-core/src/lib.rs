use hdrhistogram::Histogram;
use reqwest::ClientBuilder;
use std::collections::HashMap;
use std::io::Read;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use tokio::sync::Mutex;

pub mod error;

pub mod config;
pub use config::{Config, Method};

pub mod time;
use time::Microseconds;

/// Default maximum number of HTTP connections used
pub const MAX_CONNS: usize = 12;

/// Run load tests with the given configuration
pub fn run<C: Into<Config>>(config: C) {
    let config: Config = config.into();
    let should_exit = Arc::new(AtomicBool::new(false));
    let should_exit_clone = should_exit.clone();

    ctrlc::set_handler(move || {
        let previously_set = should_exit_clone.fetch_or(true, Ordering::SeqCst);

        if previously_set {
            std::process::exit(130);
        }
    })
    .expect("Error setting signal handler");

    let (iterations, duration) = config.iterations_and_duration();

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

    let mut cert = None;
    if let Some(cert_file) = config.ca_cert.as_deref() {
        let mut buf = Vec::new();
        std::fs::File::open(cert_file)
            .unwrap_or_else(|_| panic!("Could not open {}", cert_file))
            .read_to_end(&mut buf)
            .unwrap_or_else(|_| panic!("Could not read file {}", cert_file));
        cert = Some(
            reqwest::Certificate::from_pem(&buf)
                .unwrap_or_else(|_| panic!("Could not convert file to PEM certificate")),
        );
    }

    for _ in 0..config.connections {
        let mut client = ClientBuilder::new().danger_accept_invalid_certs(config.insecure);

        if let Some(cert) = cert.clone() {
            client = client.add_root_certificate(cert);
        }

        let client = client.build().unwrap();

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
                    Method::Get => client.get(&url),
                    Method::Post => client.post(&url),
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

    let elapsed_us = test_start_time.elapsed().unwrap().as_micros() as f64;
    print_results(
        times,
        elapsed_us,
        errors.load(Ordering::Relaxed),
        passes.load(Ordering::Relaxed),
    );
}

fn print_results(times: Histogram<u64>, elapsed_us: f64, errors: usize, passes: usize) {
    let iterations = passes + errors;
    let rps = (iterations as f64 / (elapsed_us / 1_000_000.0)) as usize;

    println!("total time: {}", Microseconds(elapsed_us));
    print!("errors: {}/{}", errors, iterations);

    if errors > 0 {
        println!(" ({:.2}%)", (errors as f64 / iterations as f64) * 100.0);
    } else {
        println!();
    }
    println!("throughput: {} req./s", rps,);

    println!(
        "response times:\n\tmean\t{}\n\tst.dev\t{}\n\tmin\t{}\n\tmax\t{}",
        Microseconds(times.mean()),
        Microseconds(times.stdev()),
        Microseconds(times.min() as f64),
        Microseconds(times.max() as f64),
    );

    println!(
        "latencies:\n\t50%\t{}\n\t75%\t{}\n\t90%\t{}\n\t95%\t{}\n\t99%\t{}\n\t99.9%\t{}",
        Microseconds(times.value_at_quantile(0.5) as f64),
        Microseconds(times.value_at_quantile(0.75) as f64),
        Microseconds(times.value_at_quantile(0.9) as f64),
        Microseconds(times.value_at_quantile(0.95) as f64),
        Microseconds(times.value_at_quantile(0.99) as f64),
        Microseconds(times.value_at_quantile(0.999) as f64),
    );
}
