use std::process::Command;

const EXE: &str = env!("CARGO_BIN_EXE_inquisitor");

#[test]
fn can_do_one_request() {
    let endpoint = "/hitme";
    let url = mockito::server_url();
    let _m = mockito::mock("GET", endpoint)
        .with_status(200)
        .with_header("content-type", "text/plain")
        .with_body("I was hit.")
        .create();

    let target = format!("{}{}", url, endpoint);

    let output = Command::new(EXE)
        .args([&target, "-n", "1"])
        .output()
        .expect("failed to execute `inquisitor` process");

    let str_out = String::from_utf8(output.stdout).unwrap();
    assert!(str_out.contains("errors: 0/1"));
}
