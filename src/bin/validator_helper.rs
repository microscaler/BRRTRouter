use brrtrouter::validator::{fail_if_issues, print_issues, ValidationIssue};

fn main() {
    let mut args = std::env::args().skip(1);
    match args.next().as_deref() {
        Some("print") => {
            let issues = sample_issues();
            print_issues(&issues);
        }
        Some("fail") => {
            let issues = sample_issues();
            fail_if_issues(issues);
        }
        _ => {
            eprintln!("unknown mode");
        }
    }
}

fn sample_issues() -> Vec<ValidationIssue> {
    vec![
        ValidationIssue::new("loc1", "Error", "message1"),
        ValidationIssue::new("loc2", "Warning", "message2"),
    ]
}
