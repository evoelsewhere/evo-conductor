use std::fs;
use std::path::{Path, PathBuf};

#[test]
fn production_api_routes_can_only_be_registered_by_the_classified_generator() {
    let http_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/http");
    let allowlisted = http_root.join("authorization/classified_router.rs");
    let mut violations = vec![];
    scan(&http_root, &allowlisted, &mut violations);

    assert!(
        violations.is_empty(),
        "raw API route registration bypasses the classified catalog:\n{}",
        violations.join("\n")
    );
}

fn scan(directory: &Path, allowlisted: &Path, violations: &mut Vec<String>) {
    for entry in fs::read_dir(directory).expect("read server HTTP source") {
        let path = entry.expect("directory entry").path();
        if path.is_dir() {
            scan(&path, allowlisted, violations);
            continue;
        }
        if path.extension().and_then(|extension| extension.to_str()) != Some("rs")
            || path == allowlisted
        {
            continue;
        }

        let source = fs::read_to_string(&path).expect("read Rust source");
        for (index, line) in source.lines().enumerate() {
            if line.contains(".route(") || line.contains(".merge(") {
                violations.push(format!("{}:{}:{line}", path.display(), index + 1));
            }
        }
    }
}
