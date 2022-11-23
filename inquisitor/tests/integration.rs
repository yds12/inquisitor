use std::process::Command;

const EXE: &str = env!("CARGO_BIN_EXE_inquisitor");

#[test]
fn can_get_version() {
    let out = get_output(&["--version"]);
    assert!(out.contains(env!("CARGO_PKG_VERSION")));
}

#[test]
fn can_do_one_request() {
    let out = get_output(&["-n", "1"]);
    assert!(out.contains("errors: 0/"));
}

#[test]
fn can_print_response() {
    let out = get_output(&["-n", "1", "--print-response"]);
    assert!(out.contains("I was hit"));
}

#[test]
fn duration_works() {
    let out = get_output(&["-d", "1s"]);
    let re = regex::Regex::new("total time: (.*) s").unwrap();
    let time: f64 = re
        .captures(&out)
        .unwrap()
        .get(1)
        .unwrap()
        .as_str()
        .parse()
        .unwrap();

    assert!(time > 0.8);
    assert!(time < 1.2);
}

fn get_output(args: &[&str]) -> String {
    let endpoint = "/hitme";
    let url = mockito::server_url();
    let _m = mockito::mock("GET", endpoint)
        .with_status(200)
        .with_header("content-type", "text/plain")
        .with_body("I was hit.")
        .create();

    let target = format!("{}{}", url, endpoint);

    let output = Command::new(EXE)
        .arg(target)
        .args(args)
        .output()
        .expect("failed to execute `inquisitor` process");

    String::from_utf8(output.stdout).unwrap()
}
