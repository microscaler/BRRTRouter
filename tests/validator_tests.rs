use std::process::Command;

#[test]
fn test_print_issues_output() {
    let exe = env!("CARGO_BIN_EXE_validator_helper");
    let output = Command::new(exe)
        .arg("print")
        .output()
        .expect("failed to run helper");
    assert!(output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("OpenAPI spec validation failed. 2 issue(s) found"));
    assert!(stderr.contains("[Error] loc1: message1"));
    assert!(stderr.contains("[Warning] loc2: message2"));
    assert!(stderr.contains("Please fix the issues"));
}

#[test]
fn test_fail_if_issues_exit_code_and_output() {
    let exe = env!("CARGO_BIN_EXE_validator_helper");
    let output = Command::new(exe)
        .arg("fail")
        .output()
        .expect("failed to run helper");
    assert!(!output.status.success());
    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("[Error] loc1: message1"));
    assert!(stderr.contains("[Warning] loc2: message2"));
}
