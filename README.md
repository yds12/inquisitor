# Inquisitor

Simple and fast HTTP load testing tool written in Rust.

This project is currently in its infancy and is very much a work in progress.

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

This will hit the URL specified, `-n` number of times, using a pool of `-c` HTTP
connections (in parallel, one `tokio` task per connection). These parameters
need to be adjusted according to your environment.

To display the help, which will have up to date information about all the
command line parameters, type:

    $ inquisitor --help

## Links

* Documentation: [docs.rs](https://docs.rs/inquisitor/latest)
* Crate: [crates.io](https://crates.io/crates/inquisitor) and [lib.rs](https://lib.rs/crates/inquisitor)
* Repository: [Github](https://github.com/yds12/inquisitor)
