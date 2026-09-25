//! Renders a `PresentationReport` as a real, native `.xlsx` workbook --
//! native Excel chart objects (editable in Excel), not chart images.

use conductor_domain::PresentationReport;
use rust_xlsxwriter::{Chart, ChartType, Format, Workbook, XlsxError};

const HEADER_ROW: u32 = 0;
const FIRST_DATA_ROW: u32 = 1;

pub fn render_xlsx(report: &PresentationReport) -> Result<Vec<u8>, XlsxError> {
    let mut workbook = Workbook::new();
    write_overview_sheet(&mut workbook, report)?;
    if !report.members.is_empty() {
        write_members_sheet(&mut workbook, report)?;
    }
    if !report.models.is_empty() {
        write_models_sheet(&mut workbook, report)?;
    }
    if !report.jira_tasks.is_empty() {
        write_jira_sheet(&mut workbook, report)?;
    }
    if !report.daily.is_empty() {
        write_trend_sheet(&mut workbook, report)?;
    }
    workbook.save_to_buffer()
}

fn usd(micros: u64) -> f64 {
    micros as f64 / 1_000_000.0
}

fn header_format() -> Format {
    Format::new().set_bold()
}

fn write_overview_sheet(
    workbook: &mut Workbook,
    report: &PresentationReport,
) -> Result<(), XlsxError> {
    let sheet = workbook.add_worksheet().set_name("Overview")?;
    let bold = header_format();
    let title = format!(
        "{} — usage report ({} to {})",
        report.project_name,
        report.window.from.format("%Y-%m-%d"),
        report.window.to.format("%Y-%m-%d"),
    );
    sheet.write_with_format(0, 0, &title, &bold)?;

    let callouts = &report.callouts;
    let mut row = 2u32;
    let mut kpi = |label: &str, value: String, sheet: &mut rust_xlsxwriter::Worksheet| {
        sheet.write_with_format(row, 0, label, &bold).ok();
        sheet.write(row, 1, value).ok();
        row += 1;
    };

    kpi(
        "Total cost (USD)",
        format!("{:.2}", usd(report.totals.estimated_cost_usd_micros)),
        sheet,
    );
    kpi(
        "Change vs. previous period",
        match callouts.cost_change_pct {
            Some(pct) => format!("{pct:+.1}%"),
            None => "n/a (no prior spend)".to_string(),
        },
        sheet,
    );
    kpi(
        "Cache savings (USD)",
        format!("{:.2}", usd(callouts.cache_savings_usd_micros)),
        sheet,
    );
    kpi(
        "Cache savings change",
        match callouts.cache_savings_change_pct {
            Some(pct) => format!("{pct:+.1}%"),
            None => "n/a".to_string(),
        },
        sheet,
    );
    kpi(
        "Unpriced calls",
        format!("{:.2}%", callouts.unpriced_ratio_bps as f64 / 100.0),
        sheet,
    );
    kpi("Total tokens", report.totals.total_tokens.to_string(), sheet);
    row += 1;

    write_top_entries_block(sheet, &bold, &mut row, "Top members by spend", &callouts.top_members)?;
    write_top_entries_block(sheet, &bold, &mut row, "Top models by spend", &callouts.top_models)?;
    write_top_entries_block(sheet, &bold, &mut row, "Top Jira tasks by spend", &callouts.top_tasks)?;

    if !callouts.cost_outlier_tasks.is_empty() {
        sheet.write_with_format(row, 0, "Possible rework (cost outliers)", &bold)?;
        row += 1;
        sheet.write_with_format(row, 0, "Issue", &bold)?;
        sheet.write_with_format(row, 1, "Type", &bold)?;
        sheet.write_with_format(row, 2, "Cost (USD)", &bold)?;
        sheet.write_with_format(row, 3, "x type average", &bold)?;
        row += 1;
        for outlier in &callouts.cost_outlier_tasks {
            sheet.write(row, 0, format!("{} — {}", outlier.issue_key, outlier.title))?;
            sheet.write(row, 1, &outlier.resolved_type)?;
            sheet.write(row, 2, usd(outlier.total_cost_usd_micros))?;
            sheet.write(row, 3, format!("{:.1}x", outlier.times_the_type_average))?;
            row += 1;
        }
    }

    sheet.autofit();
    Ok(())
}

fn write_top_entries_block(
    sheet: &mut rust_xlsxwriter::Worksheet,
    bold: &Format,
    row: &mut u32,
    title: &str,
    entries: &[conductor_domain::TopEntry],
) -> Result<(), XlsxError> {
    if entries.is_empty() {
        return Ok(());
    }
    sheet.write_with_format(*row, 0, title, bold)?;
    *row += 1;
    for entry in entries {
        sheet.write(*row, 0, &entry.label)?;
        sheet.write(*row, 1, usd(entry.total_cost_usd_micros))?;
        *row += 1;
    }
    *row += 1;
    Ok(())
}

fn write_members_sheet(
    workbook: &mut Workbook,
    report: &PresentationReport,
) -> Result<(), XlsxError> {
    let sheet = workbook.add_worksheet().set_name("Members")?;
    let bold = header_format();
    let headers = ["Member", "Role", "Calls", "Total tokens", "Cost (USD)", "Cache saved (USD)"];
    for (col, header) in headers.iter().enumerate() {
        sheet.write_with_format(HEADER_ROW, col as u16, *header, &bold)?;
    }
    for (index, row) in report.members.iter().enumerate() {
        let r = FIRST_DATA_ROW + index as u32;
        sheet.write(r, 0, &row.display_name)?;
        sheet.write(r, 1, row.primary_role.as_str())?;
        sheet.write(r, 2, row.calls)?;
        sheet.write(r, 3, row.total_tokens)?;
        sheet.write(r, 4, usd(row.total_cost_usd_micros))?;
        sheet.write(r, 5, usd(row.cache_savings_usd_micros))?;
    }
    let last_row = FIRST_DATA_ROW + report.members.len() as u32 - 1;

    let mut chart = Chart::new(ChartType::Bar);
    chart
        .add_series()
        .set_categories(("Members", FIRST_DATA_ROW, 0, last_row, 0))
        .set_values(("Members", FIRST_DATA_ROW, 4, last_row, 4))
        .set_name("Cost (USD)");
    chart.title().set_name("Spend by member");
    sheet.insert_chart(HEADER_ROW, 8, &chart)?;
    sheet.autofit();
    Ok(())
}

fn write_models_sheet(
    workbook: &mut Workbook,
    report: &PresentationReport,
) -> Result<(), XlsxError> {
    let sheet = workbook.add_worksheet().set_name("Models")?;
    let bold = header_format();
    let headers = [
        "Provider", "Model", "Calls", "Unpriced calls", "Total tokens", "Cost (USD)",
        "Cache saved (USD)",
    ];
    for (col, header) in headers.iter().enumerate() {
        sheet.write_with_format(HEADER_ROW, col as u16, *header, &bold)?;
    }
    for (index, row) in report.models.iter().enumerate() {
        let r = FIRST_DATA_ROW + index as u32;
        sheet.write(r, 0, &row.provider)?;
        sheet.write(r, 1, &row.model)?;
        sheet.write(r, 2, row.calls)?;
        sheet.write(r, 3, row.unpriced_calls)?;
        sheet.write(r, 4, row.total_tokens)?;
        sheet.write(r, 5, usd(row.total_cost_usd_micros))?;
        sheet.write(r, 6, usd(row.cache_savings_usd_micros))?;
    }
    let last_row = FIRST_DATA_ROW + report.models.len() as u32 - 1;

    let mut chart = Chart::new(ChartType::Pie);
    chart
        .add_series()
        .set_categories(("Models", FIRST_DATA_ROW, 1, last_row, 1))
        .set_values(("Models", FIRST_DATA_ROW, 5, last_row, 5))
        .set_name("Cost share by model");
    chart.title().set_name("Cost share by model");
    sheet.insert_chart(HEADER_ROW, 9, &chart)?;
    sheet.autofit();
    Ok(())
}

fn write_jira_sheet(
    workbook: &mut Workbook,
    report: &PresentationReport,
) -> Result<(), XlsxError> {
    let sheet = workbook.add_worksheet().set_name("Jira Tasks")?;
    let bold = header_format();
    let headers = [
        "Issue", "Title", "Type", "Status", "Tracked", "Calls", "Total tokens", "Cost (USD)",
        "Cache saved (USD)",
    ];
    for (col, header) in headers.iter().enumerate() {
        sheet.write_with_format(HEADER_ROW, col as u16, *header, &bold)?;
    }
    for (index, row) in report.jira_tasks.iter().enumerate() {
        let r = FIRST_DATA_ROW + index as u32;
        sheet.write(r, 0, &row.issue_key)?;
        sheet.write(r, 1, &row.title)?;
        sheet.write(r, 2, &row.resolved_type)?;
        sheet.write(r, 3, &row.status)?;
        sheet.write(r, 4, if row.precise { "Yes" } else { "No" })?;
        sheet.write(r, 5, row.calls)?;
        sheet.write(r, 6, row.total_tokens)?;
        sheet.write(r, 7, usd(row.total_cost_usd_micros))?;
        sheet.write(r, 8, usd(row.cache_savings_usd_micros))?;
    }
    sheet.autofit();
    Ok(())
}

fn write_trend_sheet(
    workbook: &mut Workbook,
    report: &PresentationReport,
) -> Result<(), XlsxError> {
    let sheet = workbook.add_worksheet().set_name("Daily Trend")?;
    let bold = header_format();
    let headers = ["Date", "Total tokens", "Cost (USD)", "Unpriced calls"];
    for (col, header) in headers.iter().enumerate() {
        sheet.write_with_format(HEADER_ROW, col as u16, *header, &bold)?;
    }
    for (index, day) in report.daily.iter().enumerate() {
        let r = FIRST_DATA_ROW + index as u32;
        sheet.write(r, 0, &day.date)?;
        sheet.write(r, 1, day.tokens_in + day.tokens_out)?;
        sheet.write(r, 2, usd(day.estimated_cost_usd_micros))?;
        sheet.write(r, 3, day.unpriced_model_calls)?;
    }
    let last_row = FIRST_DATA_ROW + report.daily.len() as u32 - 1;

    let mut chart = Chart::new(ChartType::Line);
    chart
        .add_series()
        .set_categories(("Daily Trend", FIRST_DATA_ROW, 0, last_row, 0))
        .set_values(("Daily Trend", FIRST_DATA_ROW, 2, last_row, 2))
        .set_name("Daily cost (USD)");
    chart.title().set_name("Cost over time");
    sheet.insert_chart(HEADER_ROW, 6, &chart)?;
    sheet.autofit();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::report_export::fixtures::sample_report;
    use std::io::{Cursor, Read};
    use zip::ZipArchive;

    #[test]
    fn renders_a_well_formed_xlsx_zip_with_every_sheet() {
        let bytes = render_xlsx(&sample_report()).expect("xlsx should render");
        assert!(!bytes.is_empty());

        let mut archive = ZipArchive::new(Cursor::new(bytes)).expect("valid zip archive");
        let names: Vec<String> = (0..archive.len())
            .map(|i| archive.by_index(i).unwrap().name().to_string())
            .collect();

        assert!(names.contains(&"[Content_Types].xml".to_string()));
        assert!(names.contains(&"xl/workbook.xml".to_string()));
        for expected in [
            "xl/worksheets/sheet1.xml",
            "xl/worksheets/sheet2.xml",
            "xl/worksheets/sheet3.xml",
            "xl/worksheets/sheet4.xml",
            "xl/worksheets/sheet5.xml",
        ] {
            assert!(names.contains(&expected.to_string()), "missing {expected} in {names:?}");
        }
        assert!(
            names.iter().any(|name| name.starts_with("xl/charts/")),
            "expected at least one native chart part, found {names:?}"
        );

        let mut workbook_xml = String::new();
        archive
            .by_name("xl/workbook.xml")
            .unwrap()
            .read_to_string(&mut workbook_xml)
            .unwrap();
        for sheet_name in ["Overview", "Members", "Models", "Jira Tasks", "Daily Trend"] {
            assert!(
                workbook_xml.contains(sheet_name),
                "expected sheet {sheet_name:?} in workbook.xml"
            );
        }
    }
}
