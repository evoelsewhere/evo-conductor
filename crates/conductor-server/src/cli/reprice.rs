use crate::core::reprice;
use crate::AppState;

use super::{project_id, Flags};

pub async fn run(args: &[String], state: &AppState) -> anyhow::Result<()> {
    let flags = Flags::parse(args, &["estimate-pre-catalog"])?;
    let basis = if flags.is_set("estimate-pre-catalog") {
        reprice::Basis::EarliestKnownFallback
    } else {
        reprice::Basis::InForceOnly
    };
    let report = reprice::run(&state.db, project_id(state).await?, basis).await?;
    // Printed rather than logged: an operator ran this to read the numbers.
    println!(
        "repriced: examined={} in_force={} estimated={} unpriced={}",
        report.examined, report.priced_in_force, report.priced_from_earliest, report.left_unpriced
    );
    Ok(())
}
