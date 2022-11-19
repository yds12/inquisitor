![checks](https://github.com/yds12/inquisitor/actions/workflows/checks.yml/badge.svg)
![tests](https://github.com/yds12/inquisitor/actions/workflows/tests.yml/badge.svg)

# Inquisitor

Simple and fast HTTP load testing tool written in Rust.

This project is currently in its infancy and is very much a work in progress.

## Building and Installing

### From Source

Clone the repo and build:

    $ git clone https://github.com/yds12/inquisitor
    $ cd inquisitor
    $ cargo build --release

### With Cargo

Install via `cargo` with:

    $ cargo install inquisitor

## Running

As an example, you can run with:

    $ inquisitor -n 1000 -c 10 https://localhost:8080/test

This will hit the URL specified, (at least and approximately) `-n` number of
times, using a pool of `-c` HTTP
connections (in parallel, one `tokio` task per connection). These parameters
need to be adjusted according to your environment.

Another way to run the tests is limiting by duration instead of total number of
requests, via the `-d` parameter (below we limit it to 15 seconds):

    $ inquisitor -d 15s https://localhost:8080/test

Other useful option is `-k` for insecure connections, ignoring TLS certificates.

You can also do POST requests (with `-b` for the request body):

    $ inquisitor -d 1m --method post -b "hello" https://localhost:8080/test

To set the request headers, you can use the `-H` option (once per header):

    $ inquisitor -d 1m --method post -b "hello" \
    -H "Content-Type:text/plain" -H "User-Agent:Inquisitor/8.0" \
    https://localhost:8080/test

For more useful options, type:

    $ inquisitor --help

Here's an example output:

    $ inquisitor -d 20s https://localhost:8080/test
    total time: 20.0 s
    errors: 0/651526
    throughput: 32574 req./s
    response times:
        mean	362 us
        st.dev	362 us
        min	    68 us
        max	    18.8 ms
    latencies:
        50%	    316 us
        75%	    522 us
        90%	    546 us
        95%	    571 us
        99%	    843 us
        99.9%	5.54 ms

## Motivation

There are some other tools in this category in Rust, such as
[Goose](https://github.com/tag1consulting/goose) and
[Drill](https://github.com/fcsonline/drill). *Inquisitor* is inspired more by
tools such as [wrk](https://github.com/wg/wrk) and
[siege](https://github.com/JoeDog/siege) than Goose, Drill or
[k6](https://k6.io/), which means that we want our tool to be:

* efficient: capable of generating as many requests per second (RPS) as the
  hardware allows;
* simple: we are not trying to be feature complete;
* no scripting: for this you should look into the excellent *k6* or some of the
  other tools mentioned.

From the tools that I have tried, by far the one capable of generating the
highest number of RPS is `wrk`, which in my hardware will do something around
30-70k RPS. These numbers are significantly larger than the RPS of some of the
tools I mentioned, and orders of magnitude higher than some others. This is the
main motivation of *Inquisitor*: reach the level of RPS of `wrk`, while being
written in Rust and if possible slowly add features that the comunity deems to
be useful.

## Versioning

We will follow [semantic versioning](https://semver.org/). Before 1.0, we will
bump the MINOR number when there are breaking changes in the UI/CLI (e.g.
change or removal of CLI options, change in the output format). For new features
that don't alter the program's behavior while using the previously existing CLI
options (e.g. addition of a new CLI option that don't change the program's
behavior when not used), and minor patches, we will just bump the PATCH number.

## Links

* Documentation: [docs.rs](https://docs.rs/inquisitor/latest)
* Crate: [crates.io](https://crates.io/crates/inquisitor) and [lib.rs](https://lib.rs/crates/inquisitor)
* Repository: [Github](https://github.com/yds12/inquisitor)
