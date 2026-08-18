use std::path::{Path, PathBuf};

use anyhow::{bail, Context};
use conductor_server::route_inventory::{render_route_inventory, GENERATED_ROUTE_INVENTORY_PATH};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    Check,
    Write,
}

fn main() -> anyhow::Result<()> {
    let (mode, path) = parse_args(std::env::args().skip(1))?;
    let rendered = render_route_inventory()?;

    match mode {
        Mode::Check => check(&path, &rendered),
        Mode::Write => write(&path, &rendered),
    }
}

fn parse_args(args: impl IntoIterator<Item = String>) -> anyhow::Result<(Mode, PathBuf)> {
    let mut args = args.into_iter();
    let mode = match args.next().as_deref() {
        Some("--check") => Mode::Check,
        Some("--write") => Mode::Write,
        _ => bail!(
            "usage: export_route_inventory (--check|--write) [path]\n\
             default path: {GENERATED_ROUTE_INVENTORY_PATH}"
        ),
    };
    let path = args
        .next()
        .map(PathBuf::from)
        .unwrap_or_else(default_inventory_path);
    if let Some(extra) = args.next() {
        bail!("unexpected argument: {extra}");
    }
    Ok((mode, path))
}

fn default_inventory_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(GENERATED_ROUTE_INVENTORY_PATH)
}

fn check(path: &Path, rendered: &str) -> anyhow::Result<()> {
    let existing = std::fs::read_to_string(path)
        .with_context(|| format!("read generated route inventory at {}", path.display()))?;
    if existing != rendered {
        bail!(
            "route inventory is stale: {}\nrun `cargo run -p conductor-server --bin export_route_inventory -- --write {}` and review the diff",
            path.display(),
            path.display()
        );
    }
    println!("route inventory is current: {}", path.display());
    Ok(())
}

fn write(path: &Path, rendered: &str) -> anyhow::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("create generated docs directory at {}", parent.display()))?;
    }
    std::fs::write(path, rendered)
        .with_context(|| format!("write generated route inventory at {}", path.display()))?;
    println!("wrote route inventory: {}", path.display());
    Ok(())
}
