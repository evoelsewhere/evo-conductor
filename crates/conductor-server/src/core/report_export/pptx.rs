//! Renders a `PresentationReport` as a `.pptx` deck: a fixed slide template
//! (title/KPIs, a daily-cost trend chart, top spenders) packaged as OOXML by
//! hand. No mature native-chart pptx crate exists in the Rust ecosystem
//! today (checked directly against crates.io) -- charts are rendered as PNG
//! via `plotters` and embedded as pictures: static, not editable in
//! PowerPoint, an explicit v1 trade-off documented in the report-export
//! plan.

use std::io::Write;

use conductor_domain::PresentationReport;
use plotters::prelude::*;
use zip::write::SimpleFileOptions;
use zip::ZipWriter;

const SLIDE_WIDTH_EMU: i64 = 12_192_000; // 13.333in, 16:9 widescreen
const SLIDE_HEIGHT_EMU: i64 = 6_858_000; // 7.5in

pub fn render_pptx(report: &PresentationReport) -> anyhow::Result<Vec<u8>> {
    let chart_png = render_trend_chart_png(report)?;
    let has_chart = chart_png.is_some();

    let mut slides = vec![title_slide_xml(report)];
    if has_chart {
        slides.push(trend_slide_xml());
    }
    slides.push(top_spenders_slide_xml(report));

    let mut buffer = Vec::new();
    {
        let mut zip = ZipWriter::new(std::io::Cursor::new(&mut buffer));
        let options = SimpleFileOptions::default();

        write_part(&mut zip, options, "[Content_Types].xml", &content_types_xml(slides.len()))?;
        write_part(&mut zip, options, "_rels/.rels", ROOT_RELS_XML)?;
        write_part(&mut zip, options, "docProps/core.xml", &core_props_xml(report))?;
        write_part(&mut zip, options, "docProps/app.xml", &app_props_xml(slides.len()))?;
        write_part(&mut zip, options, "ppt/presentation.xml", &presentation_xml(slides.len()))?;
        write_part(
            &mut zip,
            options,
            "ppt/_rels/presentation.xml.rels",
            &presentation_rels_xml(slides.len()),
        )?;
        write_part(&mut zip, options, "ppt/theme/theme1.xml", THEME_XML)?;
        write_part(
            &mut zip,
            options,
            "ppt/slideMasters/slideMaster1.xml",
            SLIDE_MASTER_XML,
        )?;
        write_part(
            &mut zip,
            options,
            "ppt/slideMasters/_rels/slideMaster1.xml.rels",
            SLIDE_MASTER_RELS_XML,
        )?;
        write_part(
            &mut zip,
            options,
            "ppt/slideLayouts/slideLayout1.xml",
            SLIDE_LAYOUT_XML,
        )?;
        write_part(
            &mut zip,
            options,
            "ppt/slideLayouts/_rels/slideLayout1.xml.rels",
            SLIDE_LAYOUT_RELS_XML,
        )?;

        if let Some(png) = &chart_png {
            zip.start_file("ppt/media/image1.png", options)?;
            zip.write_all(png)?;
        }

        for (index, slide_xml) in slides.iter().enumerate() {
            let number = index + 1;
            write_part(&mut zip, options, &format!("ppt/slides/slide{number}.xml"), slide_xml)?;
            let is_trend_slide = has_chart && number == 2;
            write_part(
                &mut zip,
                options,
                &format!("ppt/slides/_rels/slide{number}.xml.rels"),
                &slide_rels_xml(is_trend_slide),
            )?;
        }

        zip.finish()?;
    }
    Ok(buffer)
}

fn write_part(
    zip: &mut ZipWriter<std::io::Cursor<&mut Vec<u8>>>,
    options: SimpleFileOptions,
    path: &str,
    content: &str,
) -> anyhow::Result<()> {
    zip.start_file(path, options)?;
    zip.write_all(content.as_bytes())?;
    Ok(())
}

fn xml_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

fn usd(micros: u64) -> f64 {
    micros as f64 / 1_000_000.0
}

// ---------------------------------------------------------------------
// Chart rendering: `plotters` draws into an in-memory RGB framebuffer
// (`BitMapBackend::with_buffer`, the only in-memory constructor it has --
// the file-path one shells out to the `image` crate under the hood, which
// we'd rather call directly than round-trip through a temp file), and
// `image` encodes that buffer to PNG bytes.
// ---------------------------------------------------------------------

const CHART_WIDTH: u32 = 960;
const CHART_HEIGHT: u32 = 540;

fn render_trend_chart_png(report: &PresentationReport) -> anyhow::Result<Option<Vec<u8>>> {
    if report.daily.is_empty() {
        return Ok(None);
    }
    let mut buffer = vec![0u8; (CHART_WIDTH * CHART_HEIGHT * 3) as usize];
    {
        let root = BitMapBackend::with_buffer(&mut buffer, (CHART_WIDTH, CHART_HEIGHT))
            .into_drawing_area();
        root.fill(&WHITE)?;

        let costs: Vec<f64> = report
            .daily
            .iter()
            .map(|day| usd(day.estimated_cost_usd_micros))
            .collect();
        let max_cost = costs.iter().cloned().fold(0.0_f64, f64::max).max(1.0);

        let mut chart = ChartBuilder::on(&root)
            .caption("Daily cost (USD)", ("sans-serif", 28))
            .margin(20)
            .x_label_area_size(40)
            .y_label_area_size(60)
            .build_cartesian_2d(0..report.daily.len().saturating_sub(1).max(1), 0.0..(max_cost * 1.15))?;

        chart
            .configure_mesh()
            .x_labels(report.daily.len().min(10))
            .x_label_formatter(&|index| {
                report
                    .daily
                    .get(*index)
                    .map(|day| day.date.clone())
                    .unwrap_or_default()
            })
            .y_desc("USD")
            .draw()?;

        chart.draw_series(LineSeries::new(
            costs.iter().enumerate().map(|(i, cost)| (i, *cost)),
            &BLUE,
        ))?;
        chart.draw_series(
            costs
                .iter()
                .enumerate()
                .map(|(i, cost)| Circle::new((i, *cost), 3, BLUE.filled())),
        )?;

        root.present()?;
    }

    let rgb = image::RgbImage::from_raw(CHART_WIDTH, CHART_HEIGHT, buffer)
        .ok_or_else(|| anyhow::anyhow!("chart framebuffer did not match its declared size"))?;
    let mut png_bytes = Vec::new();
    image::DynamicImage::ImageRgb8(rgb)
        .write_to(&mut std::io::Cursor::new(&mut png_bytes), image::ImageFormat::Png)?;
    Ok(Some(png_bytes))
}

// ---------------------------------------------------------------------
// Slide content
// ---------------------------------------------------------------------

fn text_box_xml(shape_id: u32, x: i64, y: i64, cx: i64, cy: i64, lines: &[(String, bool, i32)]) -> String {
    // `lines`: (text, bold, font_size_hundredths) -- PowerPoint sizes are in
    // hundredths of a point (`sz="2800"` = 28pt).
    let paragraphs: String = lines
        .iter()
        .map(|(text, bold, size)| {
            format!(
                r#"<a:p><a:r><a:rPr lang="en-US" sz="{size}" b="{bold}" dirty="0"/><a:t>{text}</a:t></a:r></a:p>"#,
                text = xml_escape(text),
                bold = if *bold { "1" } else { "0" },
                size = size,
            )
        })
        .collect();
    format!(
        r#"<p:sp><p:nvSpPr><p:cNvPr id="{shape_id}" name="TextBox {shape_id}"/><p:cNvSpPr txBox="1"/><p:nvPr/></p:nvSpPr>
<p:spPr><a:xfrm><a:off x="{x}" y="{y}"/><a:ext cx="{cx}" cy="{cy}"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom><a:noFill/></p:spPr>
<p:txBody><a:bodyPr wrap="square"><a:normAutofit/></a:bodyPr><a:lstStyle/>{paragraphs}</p:txBody></p:sp>"#
    )
}

fn slide_wrapper(shapes: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<p:sld xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main">
<p:cSld><p:spTree>
<p:nvGrpSpPr><p:cNvPr id="1" name=""/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr>
<p:grpSpPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="0" cy="0"/><a:chOff x="0" y="0"/><a:chExt cx="0" cy="0"/></a:xfrm></p:grpSpPr>
{shapes}
</p:spTree></p:cSld>
<p:clrMapOvr><a:masterClrMapping/></p:clrMapOvr>
</p:sld>"#
    )
}

fn title_slide_xml(report: &PresentationReport) -> String {
    let title = format!("{} — usage report", report.project_name);
    let subtitle = format!(
        "{} to {}",
        report.window.from.format("%Y-%m-%d"),
        report.window.to.format("%Y-%m-%d"),
    );
    let callouts = &report.callouts;
    let change = match callouts.cost_change_pct {
        Some(pct) => format!("{pct:+.1}% vs. previous period"),
        None => "no prior period to compare".to_string(),
    };
    let lines = [
        (title, true, 3200),
        (subtitle, false, 1800),
        (String::new(), false, 1400),
        (format!("Total cost: ${:.2}", usd(report.totals.estimated_cost_usd_micros)), true, 2000),
        (change, false, 1600),
        (
            format!(
                "Cache savings: ${:.2} ({})",
                usd(callouts.cache_savings_usd_micros),
                callouts
                    .cache_savings_change_pct
                    .map(|pct| format!("{pct:+.1}%"))
                    .unwrap_or_else(|| "n/a".to_string()),
            ),
            false,
            1600,
        ),
        (
            format!("Unpriced calls: {:.2}%", callouts.unpriced_ratio_bps as f64 / 100.0),
            false,
            1600,
        ),
    ];
    let shapes = text_box_xml(2, 685_800, 685_800, SLIDE_WIDTH_EMU - 2 * 685_800, SLIDE_HEIGHT_EMU - 2 * 685_800, &lines);
    slide_wrapper(&shapes)
}

fn trend_slide_xml() -> String {
    let picture = format!(
        r#"<p:pic><p:nvPicPr><p:cNvPr id="2" name="Trend chart"/><p:cNvPicPr><a:picLocks noChangeAspect="1"/></p:cNvPicPr><p:nvPr/></p:nvPicPr>
<p:blipFill><a:blip r:embed="rId1"/><a:stretch><a:fillRect/></a:stretch></p:blipFill>
<p:spPr><a:xfrm><a:off x="685800" y="914400"/><a:ext cx="10820400" cy="6086475"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom></p:spPr></p:pic>"#
    );
    let title = text_box_xml(3, 685_800, 274_320, SLIDE_WIDTH_EMU - 2 * 685_800, 548_640, &[("Usage trend".to_string(), true, 2400)]);
    slide_wrapper(&format!("{title}\n{picture}"))
}

fn top_spenders_slide_xml(report: &PresentationReport) -> String {
    let mut lines: Vec<(String, bool, i32)> = vec![("Top spenders".to_string(), true, 2800)];
    for (heading, entries) in [
        ("Members", &report.callouts.top_members),
        ("Models", &report.callouts.top_models),
        ("Jira tasks", &report.callouts.top_tasks),
    ] {
        if entries.is_empty() {
            continue;
        }
        lines.push((heading.to_string(), true, 1800));
        for entry in entries.iter().take(5) {
            lines.push((
                format!("{} — ${:.2}", entry.label, usd(entry.total_cost_usd_micros)),
                false,
                1400,
            ));
        }
    }
    for outlier in report.callouts.cost_outlier_tasks.iter().take(3) {
        if lines.len() == 1 {
            lines.push(("Possible rework".to_string(), true, 1800));
        }
        lines.push((
            format!(
                "{} — {:.1}x the {} average",
                outlier.issue_key, outlier.times_the_type_average, outlier.resolved_type
            ),
            false,
            1400,
        ));
    }
    let shapes = text_box_xml(2, 685_800, 685_800, SLIDE_WIDTH_EMU - 2 * 685_800, SLIDE_HEIGHT_EMU - 2 * 685_800, &lines);
    slide_wrapper(&shapes)
}

// ---------------------------------------------------------------------
// Fixed package parts
// ---------------------------------------------------------------------

fn content_types_xml(slide_count: usize) -> String {
    let overrides: String = (1..=slide_count)
        .map(|n| {
            format!(
                r#"<Override PartName="/ppt/slides/slide{n}.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.slide+xml"/>"#
            )
        })
        .collect();
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
<Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
<Default Extension="xml" ContentType="application/xml"/>
<Default Extension="png" ContentType="image/png"/>
<Override PartName="/ppt/presentation.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.presentation.main+xml"/>
<Override PartName="/ppt/slideMasters/slideMaster1.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.slideMaster+xml"/>
<Override PartName="/ppt/slideLayouts/slideLayout1.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.slideLayout+xml"/>
<Override PartName="/ppt/theme/theme1.xml" ContentType="application/vnd.openxmlformats-officedocument.theme+xml"/>
<Override PartName="/docProps/core.xml" ContentType="application/vnd.openxmlformats-package.core-properties+xml"/>
<Override PartName="/docProps/app.xml" ContentType="application/vnd.openxmlformats-officedocument.extended-properties+xml"/>
{overrides}
</Types>"#
    )
}

const ROOT_RELS_XML: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
<Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="ppt/presentation.xml"/>
<Relationship Id="rId2" Type="http://schemas.openxmlformats.org/package/2006/relationships/metadata/core-properties" Target="docProps/core.xml"/>
<Relationship Id="rId3" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/extended-properties" Target="docProps/app.xml"/>
</Relationships>"#;

fn core_props_xml(report: &PresentationReport) -> String {
    let now = chrono::Utc::now().to_rfc3339();
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<cp:coreProperties xmlns:cp="http://schemas.openxmlformats.org/package/2006/metadata/core-properties" xmlns:dc="http://purl.org/dc/elements/1.1/" xmlns:dcterms="http://purl.org/dc/terms/" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance">
<dc:title>{title}</dc:title>
<dc:creator>Evo Conductor</dc:creator>
<cp:lastModifiedBy>Evo Conductor</cp:lastModifiedBy>
<dcterms:created xsi:type="dcterms:W3CDTF">{now}</dcterms:created>
<dcterms:modified xsi:type="dcterms:W3CDTF">{now}</dcterms:modified>
</cp:coreProperties>"#,
        title = xml_escape(&format!("{} — usage report", report.project_name)),
    )
}

fn app_props_xml(slide_count: usize) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Properties xmlns="http://schemas.openxmlformats.org/officeDocument/2006/extended-properties" xmlns:vt="http://schemas.openxmlformats.org/officeDocument/2006/docPropsVTypes">
<Application>Evo Conductor</Application>
<Slides>{slide_count}</Slides>
</Properties>"#
    )
}

fn presentation_xml(slide_count: usize) -> String {
    let slide_ids: String = (0..slide_count)
        .map(|i| format!(r#"<p:sldId id="{}" r:id="rId{}"/>"#, 256 + i, i + 2))
        .collect();
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<p:presentation xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main">
<p:sldMasterIdLst><p:sldMasterId id="2147483648" r:id="rId1"/></p:sldMasterIdLst>
<p:sldIdLst>{slide_ids}</p:sldIdLst>
<p:sldSz cx="{w}" cy="{h}" type="screen16x9"/>
<p:notesSz cx="6858000" cy="9144000"/>
</p:presentation>"#,
        w = SLIDE_WIDTH_EMU,
        h = SLIDE_HEIGHT_EMU,
    )
}

fn presentation_rels_xml(slide_count: usize) -> String {
    let slide_rels: String = (0..slide_count)
        .map(|i| {
            format!(
                r#"<Relationship Id="rId{}" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slide" Target="slides/slide{}.xml"/>"#,
                i + 2,
                i + 1,
            )
        })
        .collect();
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
<Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideMaster" Target="slideMasters/slideMaster1.xml"/>
{slide_rels}
</Relationships>"#
    )
}

fn slide_rels_xml(with_image: bool) -> String {
    let image_rel = if with_image {
        r#"<Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" Target="../media/image1.png"/>"#
    } else {
        ""
    };
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
<Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideLayout" Target="../slideLayouts/slideLayout1.xml"/>
{image_rel}
</Relationships>"#
    )
}

const THEME_XML: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<a:theme xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" name="Conductor">
<a:themeElements>
<a:clrScheme name="Conductor">
<a:dk1><a:sysClr val="windowText" lastClr="000000"/></a:dk1>
<a:lt1><a:sysClr val="window" lastClr="FFFFFF"/></a:lt1>
<a:dk2><a:srgbClr val="1F2933"/></a:dk2>
<a:lt2><a:srgbClr val="E4E7EB"/></a:lt2>
<a:accent1><a:srgbClr val="2563EB"/></a:accent1>
<a:accent2><a:srgbClr val="16A34A"/></a:accent2>
<a:accent3><a:srgbClr val="D97706"/></a:accent3>
<a:accent4><a:srgbClr val="DC2626"/></a:accent4>
<a:accent5><a:srgbClr val="7C3AED"/></a:accent5>
<a:accent6><a:srgbClr val="0891B2"/></a:accent6>
<a:hlink><a:srgbClr val="2563EB"/></a:hlink>
<a:folHlink><a:srgbClr val="7C3AED"/></a:folHlink>
</a:clrScheme>
<a:fontScheme name="Conductor">
<a:majorFont><a:latin typeface="Calibri"/><a:ea typeface=""/><a:cs typeface=""/></a:majorFont>
<a:minorFont><a:latin typeface="Calibri"/><a:ea typeface=""/><a:cs typeface=""/></a:minorFont>
</a:fontScheme>
<a:fmtScheme name="Conductor">
<a:fillStyleLst>
<a:solidFill><a:schemeClr val="phClr"/></a:solidFill>
<a:solidFill><a:schemeClr val="phClr"/></a:solidFill>
<a:solidFill><a:schemeClr val="phClr"/></a:solidFill>
</a:fillStyleLst>
<a:lnStyleLst>
<a:ln w="6350"><a:solidFill><a:schemeClr val="phClr"/></a:solidFill></a:ln>
<a:ln w="12700"><a:solidFill><a:schemeClr val="phClr"/></a:solidFill></a:ln>
<a:ln w="19050"><a:solidFill><a:schemeClr val="phClr"/></a:solidFill></a:ln>
</a:lnStyleLst>
<a:effectStyleLst>
<a:effectStyle><a:effectLst/></a:effectStyle>
<a:effectStyle><a:effectLst/></a:effectStyle>
<a:effectStyle><a:effectLst/></a:effectStyle>
</a:effectStyleLst>
<a:bgFillStyleLst>
<a:solidFill><a:schemeClr val="phClr"/></a:solidFill>
<a:solidFill><a:schemeClr val="phClr"/></a:solidFill>
<a:solidFill><a:schemeClr val="phClr"/></a:solidFill>
</a:bgFillStyleLst>
</a:fmtScheme>
</a:themeElements>
</a:theme>"#;

const SLIDE_MASTER_XML: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<p:sldMaster xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main">
<p:cSld><p:bg><p:bgRef idx="1001"><a:schemeClr val="bg1"/></p:bgRef></p:bg>
<p:spTree>
<p:nvGrpSpPr><p:cNvPr id="1" name=""/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr>
<p:grpSpPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="0" cy="0"/><a:chOff x="0" y="0"/><a:chExt cx="0" cy="0"/></a:xfrm></p:grpSpPr>
</p:spTree>
</p:cSld>
<p:clrMap bg1="lt1" tx1="dk1" bg2="lt2" tx2="dk2" accent1="accent1" accent2="accent2" accent3="accent3" accent4="accent4" accent5="accent5" accent6="accent6" hlink="hlink" folHlink="folHlink"/>
<p:sldLayoutIdLst><p:sldLayoutId id="2147483649" r:id="rId1"/></p:sldLayoutIdLst>
</p:sldMaster>"#;

const SLIDE_MASTER_RELS_XML: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
<Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideLayout" Target="../slideLayouts/slideLayout1.xml"/>
<Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/theme" Target="../theme/theme1.xml"/>
</Relationships>"#;

const SLIDE_LAYOUT_XML: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<p:sldLayout xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main" type="blank" preserve="1">
<p:cSld name="Blank">
<p:spTree>
<p:nvGrpSpPr><p:cNvPr id="1" name=""/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr>
<p:grpSpPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="0" cy="0"/><a:chOff x="0" y="0"/><a:chExt cx="0" cy="0"/></a:xfrm></p:grpSpPr>
</p:spTree>
</p:cSld>
<p:clrMapOvr><a:masterClrMapping/></p:clrMapOvr>
</p:sldLayout>"#;

const SLIDE_LAYOUT_RELS_XML: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
<Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideMaster" Target="../slideMasters/slideMaster1.xml"/>
</Relationships>"#;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::report_export::fixtures::sample_report;
    use quick_xml::events::Event;
    use quick_xml::Reader;
    use std::io::Cursor;
    use zip::ZipArchive;

    fn assert_well_formed_xml(label: &str, xml: &str) {
        let mut reader = Reader::from_str(xml);
        loop {
            match reader.read_event() {
                Ok(Event::Eof) => break,
                Ok(_) => {}
                Err(error) => panic!("{label} is not well-formed XML: {error}"),
            }
        }
    }

    #[test]
    fn renders_a_well_formed_pptx_zip_with_every_part() {
        let bytes = render_pptx(&sample_report()).expect("pptx should render");
        assert!(!bytes.is_empty());

        let mut archive = ZipArchive::new(Cursor::new(bytes)).expect("valid zip archive");
        let names: Vec<String> = (0..archive.len())
            .map(|i| archive.by_index(i).unwrap().name().to_string())
            .collect();

        for expected in [
            "[Content_Types].xml",
            "_rels/.rels",
            "ppt/presentation.xml",
            "ppt/_rels/presentation.xml.rels",
            "ppt/theme/theme1.xml",
            "ppt/slideMasters/slideMaster1.xml",
            "ppt/slideLayouts/slideLayout1.xml",
            "ppt/slides/slide1.xml",
            "ppt/slides/_rels/slide1.xml.rels",
            "ppt/media/image1.png",
        ] {
            assert!(names.contains(&expected.to_string()), "missing {expected} in {names:?}");
        }

        let mut xml_parts = Vec::new();
        for name in &names {
            if name.ends_with(".xml") || name.ends_with(".rels") {
                let mut content = String::new();
                std::io::Read::read_to_string(&mut archive.by_name(name).unwrap(), &mut content)
                    .unwrap();
                xml_parts.push((name.clone(), content));
            }
        }
        for (name, content) in &xml_parts {
            assert_well_formed_xml(name, content);
        }
    }

    #[test]
    fn skips_the_trend_slide_when_there_is_no_daily_data() {
        let mut report = sample_report();
        report.daily.clear();
        let bytes = render_pptx(&report).expect("pptx should still render");
        let mut archive = ZipArchive::new(Cursor::new(bytes)).unwrap();
        let names: Vec<String> = (0..archive.len())
            .map(|i| archive.by_index(i).unwrap().name().to_string())
            .collect();
        assert!(!names.contains(&"ppt/media/image1.png".to_string()));
    }
}
