//! `freespace schedule export` — Task Scheduler XML (FR-077).

use std::{fmt::Write as _, path::Path};

use freespace_core::{Error, Result, Timestamp};
use freespace_ops::{AutomationProfile, ScheduleSpec};
use serde_json::json;

use super::Outcome;

/// Exports a built-in automation profile as a Task Scheduler definition.
///
/// The XML is written as UTF-16LE with a byte-order mark, which is what
/// `schtasks /create /xml` expects.
///
/// # Errors
/// Returns [`Error::NotFound`] for an unknown profile and an error when the
/// file cannot be written.
pub fn export(profile_name: &str, out: &Path) -> Result<Outcome> {
    let profile = AutomationProfile::builtin(profile_name).ok_or_else(|| {
        Error::NotFound(format!(
            "automation profile `{profile_name}` was not found; built-in profiles: {}",
            AutomationProfile::builtin_profiles()
                .iter()
                .map(|profile| profile.name.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        ))
    })?;
    profile.validate()?;
    let xml = render_task_xml(&profile);
    write_utf16(out, &xml)?;

    let schedule = profile
        .schedule
        .as_ref()
        .map(ScheduleSpec::describe)
        .unwrap_or_else(|| "manual".to_owned());

    Ok(Outcome::new(
        json!({
            "profile": profile.name,
            "id": profile.id,
            "command": profile.command,
            "args": profile.args,
            "schedule": schedule,
            "path": out.to_string_lossy(),
        }),
        format!(
            "task `{}` written to {} ({schedule})\n",
            profile.name,
            out.display()
        ),
    ))
}

/// Renders the Task Scheduler XML document for `profile`.
#[must_use]
pub fn render_task_xml(profile: &AutomationProfile) -> String {
    let mut xml = String::new();
    xml.push_str("<?xml version=\"1.0\" encoding=\"UTF-16\"?>\n");
    xml.push_str(
        "<Task version=\"1.2\" xmlns=\"http://schemas.microsoft.com/windows/2004/02/mit/task\">\n",
    );
    let _ = writeln!(
        xml,
        "  <RegistrationInfo><Author>FreeSpace</Author><Description>{}</Description>\
         <Date>{}</Date></RegistrationInfo>",
        escape(&format!("FreeSpace automation profile `{}`", profile.name)),
        Timestamp::now().format_iso8601()
    );

    xml.push_str("  <Triggers>\n");
    if let Some(schedule) = &profile.schedule {
        xml.push_str(&trigger_xml(schedule));
    }
    xml.push_str("  </Triggers>\n");

    xml.push_str(
        "  <Principals><Principal id=\"Author\"><LogonType>InteractiveToken</LogonType>\
         <RunLevel>LeastPrivilege</RunLevel></Principal></Principals>\n",
    );
    xml.push_str(
        "  <Settings><MultipleInstancesPolicy>IgnoreNew</MultipleInstancesPolicy>\
         <DisallowStartIfOnBatteries>false</DisallowStartIfOnBatteries>\
         <StopIfGoingOnBatteries>false</StopIfGoingOnBatteries>\
         <StartWhenAvailable>true</StartWhenAvailable>\
         <ExecutionTimeLimit>PT4H</ExecutionTimeLimit><Enabled>true</Enabled></Settings>\n",
    );
    let _ = writeln!(
        xml,
        "  <Actions Context=\"Author\"><Exec><Command>{}</Command><Arguments>{}</Arguments>\
         <WorkingDirectory>{}</WorkingDirectory></Exec></Actions>",
        escape(&profile.command),
        escape(&quote_args(&profile.args)),
        escape(
            &std::env::current_dir()
                .map(|path| path.to_string_lossy().into_owned())
                .unwrap_or_default()
        )
    );
    xml.push_str("</Task>\n");

    xml
}

fn trigger_xml(schedule: &ScheduleSpec) -> String {
    let start_date = {
        let now = Timestamp::now().format_iso8601();
        now[..10.min(now.len())].to_owned()
    };
    let mut xml = String::new();

    match schedule {
        ScheduleSpec::Daily {
            start,
            interval_days,
        } => {
            let _ = writeln!(
                xml,
                "    <CalendarTrigger><StartBoundary>{start_date}T{start}:00</StartBoundary>\
                 <Enabled>true</Enabled><ScheduleByDay><DaysInterval>{interval_days}</DaysInterval>\
                 </ScheduleByDay></CalendarTrigger>"
            );
        }
        ScheduleSpec::Weekly {
            start,
            interval_weeks,
            days,
        } => {
            let days = days
                .iter()
                .map(|day| format!("<{}/>", day.task_name()))
                .collect::<String>();
            let _ = writeln!(
                xml,
                "    <CalendarTrigger><StartBoundary>{start_date}T{start}:00</StartBoundary>\
                 <Enabled>true</Enabled><ScheduleByWeek><WeeksInterval>{interval_weeks}\
                 </WeeksInterval><DaysOfWeek>{days}</DaysOfWeek></ScheduleByWeek>\
                 </CalendarTrigger>"
            );
        }
        ScheduleSpec::Monthly {
            start,
            day,
            interval_months,
        } => {
            let _ = interval_months;
            let months = [
                "January",
                "February",
                "March",
                "April",
                "May",
                "June",
                "July",
                "August",
                "September",
                "October",
                "November",
                "December",
            ]
            .iter()
            .map(|month| format!("<{month}/>"))
            .collect::<String>();
            let _ = writeln!(
                xml,
                "    <CalendarTrigger><StartBoundary>{start_date}T{start}:00</StartBoundary>\
                 <Enabled>true</Enabled><ScheduleByMonth><DaysOfMonth><Day>{day}</Day>\
                 </DaysOfMonth><Months>{months}</Months></ScheduleByMonth></CalendarTrigger>"
            );
        }
        ScheduleSpec::AtLogon => {
            xml.push_str("    <LogonTrigger><Enabled>true</Enabled></LogonTrigger>\n");
        }
        ScheduleSpec::AtStartup => {
            xml.push_str("    <BootTrigger><Enabled>true</Enabled></BootTrigger>\n");
        }
    }

    xml
}

fn quote_args(args: &[String]) -> String {
    args.iter()
        .map(|argument| {
            if argument.is_empty()
                || argument
                    .chars()
                    .any(|character| matches!(character, ' ' | '\t' | '"'))
            {
                format!("\"{}\"", argument.replace('"', "\\\""))
            } else {
                argument.clone()
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn write_utf16(path: &Path, xml: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
    }
    let mut bytes = vec![0xFF, 0xFE];
    for unit in xml.encode_utf16() {
        bytes.extend_from_slice(&unit.to_le_bytes());
    }
    std::fs::write(path, bytes)?;

    Ok(())
}

fn escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

#[cfg(test)]
mod tests {
    use super::{export, render_task_xml};
    use freespace_ops::{AutomationProfile, ScheduleSpec, Weekday};

    fn decode(path: &std::path::Path) -> String {
        let bytes = std::fs::read(path).expect("read");
        assert_eq!(&bytes[..2], &[0xFF, 0xFE], "UTF-16LE BOM");
        let units: Vec<u16> = bytes[2..]
            .chunks_exact(2)
            .map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]]))
            .collect();

        String::from_utf16(&units).expect("UTF-16")
    }

    #[test]
    fn renders_daily_weekly_and_monthly_triggers() {
        let profile = AutomationProfile::new("daily", "freespace.exe", vec!["scan".into()])
            .with_schedule(ScheduleSpec::Daily {
                start: "02:00".to_owned(),
                interval_days: 1,
            });
        assert!(render_task_xml(&profile).contains("<ScheduleByDay>"));

        let profile = profile.with_schedule(ScheduleSpec::Weekly {
            start: "03:00".to_owned(),
            interval_weeks: 2,
            days: vec![Weekday::Monday, Weekday::Friday],
        });
        let xml = render_task_xml(&profile);
        assert!(xml.contains("<WeeksInterval>2</WeeksInterval>"));
        assert!(xml.contains("<Monday/>"));

        let profile = profile.with_schedule(ScheduleSpec::Monthly {
            start: "04:00".to_owned(),
            day: 1,
            interval_months: 1,
        });
        assert!(render_task_xml(&profile).contains("<DaysOfMonth>"));
    }

    #[test]
    fn exports_a_builtin_and_rejects_unknown_profiles() {
        let directory = tempfile::tempdir().expect("temp dir");
        let path = directory.path().join("daily.xml");
        let outcome = export("daily-scan", &path).expect("export");

        assert_eq!(outcome.data["profile"], "daily-scan");
        let xml = decode(&path);
        assert!(xml.contains("<Task version=\"1.2\""));
        assert!(xml.contains("<Command>freespace.exe</Command>"));
        assert!(xml.contains("<ScheduleByDay>"));

        let error = export("missing", &path).expect_err("missing profile");
        assert_eq!(error.code().as_str(), "E_NOT_FOUND");
    }
}
