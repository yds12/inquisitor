use clap::{Parser as _, ValueEnum, AppSettings};
use reqwest::{Client, ClientBuilder};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use tokio::sync::Mutex;

const ITERATIONS: usize = 10_000;
const MAX_CONNS: usize = 12;
const CONCURRENT_TASKS: usize = 100;

#[derive(Debug, Copy, Clone, PartialEq, ValueEnum)]
pub enum Method {
    GET,
    POST
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
    /// Maximum number of tasks to be run concurrently
    #[clap(long, short = 't', default_value_t = CONCURRENT_TASKS, value_parser)]
    tasks: usize,
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

#[derive(Debug)]
struct ConnectionPool {
    pool: Vec<Option<Client>>,
}

impl ConnectionPool {
    pub fn new(max_conns: usize, ignore_certs: bool) -> Self {
        Self {
            pool: (0..max_conns)
                .into_iter()
                .map(|_| {
                    Some(
                        ClientBuilder::new()
                            .danger_accept_invalid_certs(ignore_certs)
                            .build()
                            .unwrap(),
                    )
                })
                .collect(),
        }
    }

    pub fn get_one(&mut self) -> Option<Client> {
        for slot in self.pool.iter_mut() {
            if slot.is_some() {
                return slot.take();
            }
        }

        return None;
    }

    pub fn put_back(&mut self, conn: Client) {
        for slot in self.pool.iter_mut() {
            if slot.is_none() {
                *slot = Some(conn);
                return;
            }
        }
    }
}

#[tokio::main]
async fn main() {
    let config = Config::parse();
    let pool = ConnectionPool::new(config.connections, config.ignore_certs);
    let pool_mutex = Arc::new(Mutex::new(pool));

    let times = Arc::new(Mutex::new(
        (0..config.iterations).map(|_| 0).collect::<Vec<u128>>(),
    ));
    let passes = Arc::new(AtomicUsize::new(0));
    let errors = Arc::new(AtomicUsize::new(0));

    let test_start_time = std::time::SystemTime::now();
    let batches = config.iterations / config.tasks;

    let failed_regex = if let Some(regex) = config.failed_body {
        Some(regex::Regex::new(&regex).expect("Failed to parse regex"))
    } else {
        None
    };

    let request_body = Box::leak(Box::new(config.request_body)) as &Option<_>;

    for i in 0..batches {
        let mut handles = Vec::new();

        for j in 0..config.tasks {
            let passes = passes.clone();
            let errors = errors.clone();
            let pool = pool_mutex.clone();
            let url = config.url.clone();
            let failed_regex = failed_regex.clone();
            let times = times.clone();

            let task = tokio::spawn(async move {
                let client = loop {
                    let mut pool = pool.lock().await;

                    let c = pool.get_one();
                    if c.is_some() {
                        break c;
                    }
                };

                let client = client.unwrap();

                let req_start_time = std::time::SystemTime::now();
                let response = match (config.method, request_body.as_deref()) {
                    (Method::GET, _) => client.get(url).send().await,
                    (Method::POST, Some(body)) => client.post(url).body(body).send().await,
                    (Method::POST, None) => client.post(url).send().await,
                };
                times.lock().await[i * config.tasks + j] =
                    req_start_time.elapsed().unwrap().as_micros();

                match response {
                    Ok(res) if res.status() == 200 && failed_regex.is_none() => {
                        passes.fetch_add(1, Ordering::SeqCst);
                        if config.print_response {
                            println!(
                                "Response successful. Content: {}",
                                res.text().await.unwrap()
                            );
                        }
                    }
                    Ok(res) if res.status() == 200 && failed_regex.is_some() => {
                        let body = res.text().await.unwrap();

                        if failed_regex.unwrap().is_match(&body) {
                            println!("Response is 200 but body indicates an error: {}", body);
                            errors.fetch_add(1, Ordering::SeqCst);
                        } else {
                            passes.fetch_add(1, Ordering::SeqCst);

                            if config.print_response {
                                println!("Response successful. Contents: {}", body);
                            }
                        }
                    }
                    Ok(res) if res.status() != 200 => {
                        println!("Response is not 200. Status code: {}", res.status());
                        errors.fetch_add(1, Ordering::SeqCst);
                    }
                    Err(e) => {
                        println!("Request failed: {}", e);
                        errors.fetch_add(1, Ordering::SeqCst);
                    }
                    _ => unreachable!(),
                };

                pool.lock().await.put_back(client);
            });

            handles.push(task);
        }

        for h in handles {
            h.await.unwrap();
        }
    }

    let elapsed_ns = test_start_time.elapsed().unwrap().as_nanos();
    let elapsed_ms = elapsed_ns as f64 / 1_000_000.0;
    let rps = (config.iterations as f64 / (elapsed_ms / 1_000.0)) as usize;

    let times = &mut *times.lock().await;

    println!(
        "total time: {:.3} s\nerrors: {:?}/{:?}\nthroughput: {} req./s",
        elapsed_ms / 1_000.0,
        errors,
        passes.load(Ordering::Relaxed) + errors.load(Ordering::Relaxed),
        rps,
    );

    println!(
        "response times:\n\tmean\t{:.3} ms\n\tmin\t{:.3} ms\n\tmax\t{:.3} ms",
        times.iter().sum::<u128>() as f64 / config.iterations as f64 / 1000.0,
        *times.iter().min().unwrap() as f64 / 1000.0,
        *times.iter().max().unwrap() as f64 / 1000.0,
    );

    println!(
        "latencies:\n\t50%\t{:.3} ms\n\t75%\t{:.3} ms\n\t90%\t{:.3} ms\n\t95%\t{:.3} ms\n\t99%\t{:.3} ms\n\t99.9%\t{:.3} ms",
        percentile(times, 50.0) as f64 / 1000.0,
        percentile(times, 75.0) as f64 / 1000.0,
        percentile(times, 90.0) as f64 / 1000.0,
        percentile(times, 95.0) as f64 / 1000.0,
        percentile(times, 99.0) as f64 / 1000.0,
        percentile(times, 99.9) as f64 / 1000.0,
    );
}

fn percentile(vals: &mut Vec<u128>, perc: f64) -> u128 {
    vals.sort();
    let perc_ix = ((vals.len() - 1) as f64 * (perc / 100.0) as f64) as usize;

    vals[perc_ix]
}
