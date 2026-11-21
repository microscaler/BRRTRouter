#![allow(clippy::unwrap_used, clippy::expect_used)]
//! `brrtrouter-gen generate --dry-run` writes nothing: not into the output directory (it used to write
//! `src/handlers/mod.rs` and `src/controllers/mod.rs` anyway), and not beside the spec.

use brrtrouter::generator::{generate_project_with_options, GenerationScope};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

fn pet_store_spec() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("examples")
        .join("openapi.yaml")
}

/// Every file under `dir` (relative path -> contents); directories are implied by their files, and listed when empty.
fn snapshot(dir: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    fn walk(root: &Path, dir: &Path, out: &mut BTreeMap<PathBuf, Vec<u8>>) {
        let mut empty = true;
        for entry in fs::read_dir(dir).unwrap() {
            empty = false;
            let path = entry.unwrap().path();
            if path.is_dir() {
                walk(root, &path, out);
            } else {
                out.insert(
                    path.strip_prefix(root).unwrap().to_path_buf(),
                    fs::read(&path).unwrap(),
                );
            }
        }
        if empty && dir != root {
            out.insert(
                dir.strip_prefix(root).unwrap().join("<empty dir>"),
                Vec::new(),
            );
        }
    }
    let mut out = BTreeMap::new();
    if dir.exists() {
        walk(dir, dir, &mut out);
    }
    out
}

fn generate(out: &Path, dry_run: bool) {
    generate_project_with_options(
        &pet_store_spec(),
        Some(out),
        true, // --force: a dry run with --force must still write nothing
        dry_run,
        &GenerationScope::all(),
        None,
        Some("pet_store_dry_run_gen"),
        None,
    )
    .expect("generate");
}

#[test]
fn dry_run_into_a_new_directory_creates_nothing() {
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("gen");
    generate(&out, true);
    assert!(
        !out.exists(),
        "dry run created {out:?}: {:?}",
        snapshot(&out).keys().collect::<Vec<_>>()
    );
}

#[test]
fn dry_run_leaves_an_existing_project_unchanged() {
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("gen");
    generate(&out, false);
    // Make the files a real run would rewrite differ from what it would write, so a write cannot go unnoticed.
    for rel in [
        "src/handlers/mod.rs",
        "src/controllers/mod.rs",
        "src/registry.rs",
        "src/main.rs",
        "Cargo.toml",
    ] {
        let path = out.join(rel);
        assert!(path.exists(), "{rel} was not generated");
        fs::write(
            &path,
            format!("// sentinel: {rel} as it was before the dry run\n"),
        )
        .unwrap();
    }
    let spec_dir = pet_store_spec().parent().unwrap().to_path_buf();
    let spec_dir_before: Vec<_> = fs::read_dir(&spec_dir)
        .unwrap()
        .map(|e| e.unwrap().file_name())
        .collect();

    let before = snapshot(&out);
    generate(&out, true);
    let after = snapshot(&out);

    let changed: BTreeSet<_> = before
        .keys()
        .chain(after.keys())
        .filter(|k| before.get(*k) != after.get(*k))
        .collect();
    assert!(changed.is_empty(), "dry run changed {changed:?}");
    let spec_dir_after: Vec<_> = fs::read_dir(&spec_dir)
        .unwrap()
        .map(|e| e.unwrap().file_name())
        .collect();
    assert_eq!(
        spec_dir_before.len(),
        spec_dir_after.len(),
        "dry run wrote beside the spec"
    );
}
