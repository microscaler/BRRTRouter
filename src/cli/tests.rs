//! Unit tests for CLI commands

use crate::cli::{Cli, Commands};
use clap::Parser;

#[test]
fn test_lint_command_exists() {
    // Test that the lint command can be parsed
    let cli = Cli::try_parse_from(["brrtrouter-gen", "lint", "--spec", "test.yaml"]).unwrap();

    match cli.command {
        Commands::Lint { spec, .. } => {
            assert_eq!(spec.to_string_lossy(), "test.yaml");
        }
        _ => panic!("Expected Lint command"),
    }
}

#[test]
fn test_lint_command_with_flags() {
    let cli = Cli::try_parse_from([
        "brrtrouter-gen",
        "lint",
        "--spec",
        "test.yaml",
        "--fail-on-error",
        "--errors-only",
    ])
    .unwrap();

    match cli.command {
        Commands::Lint {
            spec,
            fail_on_error,
            errors_only,
        } => {
            assert_eq!(spec.to_string_lossy(), "test.yaml");
            assert!(fail_on_error);
            assert!(errors_only);
        }
        _ => panic!("Expected Lint command"),
    }
}

#[test]
fn test_all_commands_parse() {
    // Verify all commands can be parsed
    let commands = vec![
        vec![
            "brrtrouter-gen",
            "generate",
            "--spec",
            "test.yaml",
            "--output",
            "out",
        ],
        vec![
            "brrtrouter-gen",
            "generate-stubs",
            "--spec",
            "test.yaml",
            "--output",
            "out",
        ],
        vec!["brrtrouter-gen", "lint", "--spec", "test.yaml"],
        vec!["brrtrouter-gen", "serve", "--spec", "test.yaml"],
    ];

    for args in commands {
        let cli = Cli::try_parse_from(&args);
        assert!(cli.is_ok(), "Failed to parse command: {:?}", args);
    }
}
