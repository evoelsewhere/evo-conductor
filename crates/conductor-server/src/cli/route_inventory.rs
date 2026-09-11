use std::path::Path;

use crate::route_inventory::render_route_inventory;

use super::Flags;

/// The snapshot `authorization.rs` compares the rendered manifest against.
const SNAPSHOT: &str = "docs/generated/req-004-route-inventory.json";

/// Render the route inventory, or rewrite the checked-in snapshot.
///
/// The drift test compares byte for byte, so before this the only way to
/// update the snapshot was to copy it out of an assertion failure. Writing
/// LF explicitly because the renderer always emits LF and a CRLF checkout
/// would fail the very test this feeds.
pub async fn run(args: &[String]) -> anyhow::Result<()> {
    let flags = Flags::parse(args, &["write"])?;
    let rendered = render_route_inventory()?;
    if !flags.is_set("write") {
        print!("{rendered}");
        return Ok(());
    }
    let path = Path::new(SNAPSHOT);
    let unchanged = std::fs::read_to_string(path)
        .is_ok_and(|existing| existing.replace("\r\n", "\n") == rendered);
    std::fs::write(path, rendered.as_bytes())?;
    if unchanged {
        println!("{SNAPSHOT} already current");
    } else {
        println!("{SNAPSHOT} updated");
    }
    Ok(())
}
