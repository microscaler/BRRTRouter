#[derive(Debug, Clone)]
pub struct ValidationIssue {
    pub location: String,
    pub kind: String,
    pub message: String,
}

impl ValidationIssue {
    pub fn new(location: impl Into<String>, kind: impl Into<String>, message: impl Into<String>) -> Self {
        ValidationIssue {
            location: location.into(),
            kind: kind.into(),
            message: message.into(),
        }
    }
}

pub fn print_issues(issues: &[ValidationIssue]) {
    eprintln!("\n❌ OpenAPI spec validation failed. {} issue(s) found:\n", issues.len());
    for issue in issues {
        eprintln!("[{}] {}: {}", issue.kind, issue.location, issue.message);
    }
    eprintln!("\nPlease fix the issues in your OpenAPI spec before starting the server.\n");
}

pub fn fail_if_issues(issues: Vec<ValidationIssue>) {
    if !issues.is_empty() {
        print_issues(&issues);
        std::process::exit(1);
    }
}
