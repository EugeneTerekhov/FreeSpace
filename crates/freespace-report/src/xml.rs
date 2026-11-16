//! XML report writer (`quick-xml`, contract `contracts/report-formats.md`).

use std::{
    fs::File,
    io::{BufWriter, Write},
    path::Path,
};

use freespace_core::{Error, Result};
use quick_xml::{
    Writer,
    events::{BytesDecl, BytesEnd, BytesStart, BytesText, Event},
};

use crate::report::{Cell, Report};

/// Writes a report as a standalone XML document.
///
/// # Errors
/// Returns an error when the output file cannot be created or written.
pub fn write(report: &Report<'_>, path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
    }
    let file = File::create(path)?;
    let mut writer = Writer::new_with_indent(BufWriter::new(file), b' ', 2);

    let mut root = BytesStart::new("freespace-report");
    let version = report.meta.report_version.to_string();
    root.push_attribute(("version", version.as_str()));
    event(
        &mut writer,
        Event::Decl(BytesDecl::new("1.0", Some("UTF-8"), None)),
    )?;
    event(&mut writer, Event::Start(root))?;

    write_meta(&mut writer, report)?;

    let section_count = report.sections().len().to_string();
    let mut sections = BytesStart::new("sections");
    sections.push_attribute(("count", section_count.as_str()));
    event(&mut writer, Event::Start(sections))?;
    for section in report.sections() {
        let mut element = BytesStart::new("section");
        element.push_attribute(("name", section.as_str()));
        event(&mut writer, Event::Start(element))?;

        report.for_each_row(section, |row| {
            let mut entry = BytesStart::new(section.element_name());
            for (header, cell) in section.headers().iter().zip(row.iter()) {
                let value = match cell {
                    Cell::Text(text) => text.clone(),
                    other => other.to_text(),
                };
                entry.push_attribute((*header, value.as_str()));
            }
            event(&mut writer, Event::Empty(entry)).map(|_| ())
        })?;

        event(&mut writer, Event::End(BytesEnd::new("section")))?;
    }
    event(&mut writer, Event::End(BytesEnd::new("sections")))?;

    let mut totals = BytesStart::new("totals");
    let logical = report.totals.logical.to_string();
    let on_disk = report.totals.on_disk.to_string();
    let physical = report.totals.physical.to_string();
    let ads = report.totals.ads.to_string();
    totals.push_attribute(("logical-size", logical.as_str()));
    totals.push_attribute(("on-disk-size", on_disk.as_str()));
    totals.push_attribute(("physical-size", physical.as_str()));
    totals.push_attribute(("ads", ads.as_str()));
    event(&mut writer, Event::Empty(totals))?;
    event(&mut writer, Event::End(BytesEnd::new("freespace-report")))?;

    writer.into_inner().flush().map_err(Error::Io)?;

    Ok(())
}

fn write_meta(writer: &mut Writer<BufWriter<File>>, report: &Report<'_>) -> Result<()> {
    event(writer, Event::Start(BytesStart::new("meta")))?;

    let generated_at = report.meta.generated_at.format_iso8601();
    let mut generated = BytesStart::new("generated-at");
    generated.push_attribute(("value", generated_at.as_str()));
    event(writer, Event::Empty(generated))?;

    let mut app = BytesStart::new("app-version");
    app.push_attribute(("value", report.meta.app_version.as_str()));
    event(writer, Event::Empty(app))?;

    if let Some(volume) = &report.meta.volume {
        let mut element = BytesStart::new("volume");
        element.push_attribute(("letter", volume.letter.as_str()));
        element.push_attribute(("label", volume.label.as_str()));
        element.push_attribute(("filesystem", volume.filesystem.as_str()));
        element.push_attribute(("scan-method", volume.scan_method.as_str()));
        event(writer, Event::Empty(element))?;
    }

    let mut units = BytesStart::new("units");
    units.push_attribute(("name", report.meta.units.as_str()));
    units.push_attribute(("session", report.meta.session_id.as_str()));
    event(writer, Event::Empty(units))?;

    event(writer, Event::Start(BytesStart::new("roots")))?;
    for root in &report.meta.roots {
        event(writer, Event::Start(BytesStart::new("root")))?;
        event(writer, Event::Text(BytesText::new(root)))?;
        event(writer, Event::End(BytesEnd::new("root")))?;
    }
    event(writer, Event::End(BytesEnd::new("roots")))?;

    event(writer, Event::End(BytesEnd::new("meta")))?;

    Ok(())
}

fn event(writer: &mut Writer<BufWriter<File>>, event: Event<'_>) -> Result<()> {
    writer
        .write_event(event)
        .map_err(|error| Error::Operation(format!("cannot write XML report: {error}")))
}

#[cfg(test)]
mod tests {
    use freespace_core::{Timestamp, size::SizeMetrics};

    use crate::report::{Report, ReportMeta};

    use super::write;

    #[test]
    fn writes_the_documented_skeleton() {
        let directory = tempfile::tempdir().expect("temp dir");
        let path = directory.path().join("report.xml");
        let meta = ReportMeta::new(Timestamp::UNIX_EPOCH, "1.0.0")
            .with_roots(["C:\\".to_owned()])
            .with_session("session")
            .with_units("binary");
        let report = Report::new(meta, SizeMetrics::ZERO);

        write(&report, &path).expect("write");
        let text = std::fs::read_to_string(&path).expect("read");

        assert!(text.contains("<freespace-report version=\"1\">"));
        assert!(text.contains("<generated-at"));
        assert!(text.contains("<sections count=\"0\">"));
        assert!(text.contains("<totals logical-size=\"0\""));
        assert!(text.contains("<root>C:\\</root>"));
    }
}
