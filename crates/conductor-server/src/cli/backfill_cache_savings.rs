use crate::core::reprice;
use crate::AppState;

use super::project_id;

pub async fn run(_args: &[String], state: &AppState) -> anyhow::Result<()> {
    let report = reprice::backfill_cache_savings(&state.db, project_id(state).await?).await?;
    // Printed rather than logged: an operator ran this to read the numbers.
    println!(
        "cache savings backfilled: examined={} filled_in={} unresolved={}",
        report.examined, report.filled_in, report.left_unresolved
    );
    Ok(())
}
