//! Automation profiles for unattended runs (FR-077, data-model §15).
//!
//! A profile describes one command, its arguments and an optional schedule. The
//! CLI turns a profile into a Task Scheduler XML definition; storing the profile
//! separately keeps the model testable and lets the GUI edit it later.

use std::path::PathBuf;

use freespace_core::{Error, Result, Timestamp, Uuid};
use serde::{Deserialize, Serialize};

/// Weekday used by weekly schedules.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Weekday {
    /// Monday.
    Monday,
    /// Tuesday.
    Tuesday,
    /// Wednesday.
    Wednesday,
    /// Thursday.
    Thursday,
    /// Friday.
    Friday,
    /// Saturday.
    Saturday,
    /// Sunday.
    Sunday,
}

impl Weekday {
    /// Task Scheduler element name.
    #[must_use]
    pub const fn task_name(self) -> &'static str {
        match self {
            Self::Monday => "Monday",
            Self::Tuesday => "Tuesday",
            Self::Wednesday => "Wednesday",
            Self::Thursday => "Thursday",
            Self::Friday => "Friday",
            Self::Saturday => "Saturday",
            Self::Sunday => "Sunday",
        }
    }

    /// Parses an English or Russian weekday name.
    #[must_use]
    pub fn parse_value(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "monday" | "mon" | "понедельник" | "пн" => Some(Self::Monday),
            "tuesday" | "tue" | "вторник" | "вт" => Some(Self::Tuesday),
            "wednesday" | "wed" | "среда" | "ср" => Some(Self::Wednesday),
            "thursday" | "thu" | "четверг" | "чт" => Some(Self::Thursday),
            "friday" | "fri" | "пятница" | "пт" => Some(Self::Friday),
            "saturday" | "sat" | "суббота" | "сб" => Some(Self::Saturday),
            "sunday" | "sun" | "воскресенье" | "вс" => Some(Self::Sunday),
            _ => None,
        }
    }
}

/// When an automated profile runs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum ScheduleSpec {
    /// Every `interval_days` days at `start`.
    Daily {
        /// Start time in `HH:MM` form.
        start: String,
        /// Interval in days, at least one.
        interval_days: u16,
    },
    /// Selected weekdays at `start`.
    Weekly {
        /// Start time in `HH:MM` form.
        start: String,
        /// Interval in weeks, at least one.
        interval_weeks: u16,
        /// Days of the week.
        days: Vec<Weekday>,
    },
    /// One day per month at `start`.
    Monthly {
        /// Start time in `HH:MM` form.
        start: String,
        /// Day of the month, `1..=31`.
        day: u8,
        /// Interval in months, at least one.
        interval_months: u16,
    },
    /// When the user logs on.
    AtLogon,
    /// When the machine starts.
    AtStartup,
}

impl ScheduleSpec {
    /// Validates the schedule fields.
    ///
    /// # Errors
    /// Returns [`Error::InvalidArgument`] when the values cannot be represented
    /// by Task Scheduler.
    pub fn validate(&self) -> Result<()> {
        match self {
            Self::Daily {
                start,
                interval_days,
            } => {
                validate_start(start)?;
                if *interval_days == 0 {
                    return Err(Error::InvalidArgument(
                        "`interval_days` must be at least 1".to_owned(),
                    ));
                }
            }
            Self::Weekly {
                start,
                interval_weeks,
                days,
            } => {
                validate_start(start)?;
                if *interval_weeks == 0 {
                    return Err(Error::InvalidArgument(
                        "`interval_weeks` must be at least 1".to_owned(),
                    ));
                }
                if days.is_empty() {
                    return Err(Error::InvalidArgument(
                        "a weekly schedule needs at least one weekday".to_owned(),
                    ));
                }
            }
            Self::Monthly {
                start,
                day,
                interval_months,
            } => {
                validate_start(start)?;
                if !(1..=31).contains(day) {
                    return Err(Error::InvalidArgument(
                        "`day` must be between 1 and 31".to_owned(),
                    ));
                }
                if *interval_months == 0 {
                    return Err(Error::InvalidArgument(
                        "`interval_months` must be at least 1".to_owned(),
                    ));
                }
            }
            Self::AtLogon | Self::AtStartup => {}
        }

        Ok(())
    }

    /// Human-readable description used in the CLI output.
    #[must_use]
    pub fn describe(&self) -> String {
        match self {
            Self::Daily {
                start,
                interval_days,
            } => format!("every {interval_days} day(s) at {start}"),
            Self::Weekly {
                start,
                interval_weeks,
                days,
            } => format!(
                "every {interval_weeks} week(s) on {} at {start}",
                days.iter()
                    .map(|day| day.task_name())
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            Self::Monthly {
                start,
                day,
                interval_months,
            } => format!("every {interval_months} month(s) on day {day} at {start}"),
            Self::AtLogon => "at logon".to_owned(),
            Self::AtStartup => "at startup".to_owned(),
        }
    }
}

/// One reusable automation profile.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AutomationProfile {
    /// Stable identifier.
    pub id: Uuid,
    /// Profile name used by `schedule export`.
    pub name: String,
    /// Executable or command to run.
    pub command: String,
    /// Command-line arguments.
    pub args: Vec<String>,
    /// Optional schedule.
    pub schedule: Option<ScheduleSpec>,
    /// Last run time, when the profile has run.
    pub last_run: Option<Timestamp>,
    /// Exit code of the last run.
    pub last_exit_code: Option<i32>,
    /// Where the command writes its log.
    pub log_path: PathBuf,
}

impl AutomationProfile {
    /// Builds a profile, deriving its identifier from the name.
    #[must_use]
    pub fn new(name: impl Into<String>, command: impl Into<String>, args: Vec<String>) -> Self {
        let name = name.into();
        let id = Uuid::new_v5(&Uuid::NAMESPACE_OID, name.as_bytes());

        Self {
            id,
            log_path: PathBuf::from(format!("{name}.log")),
            name,
            command: command.into(),
            args,
            schedule: None,
            last_run: None,
            last_exit_code: None,
        }
    }

    /// Attaches a schedule.
    #[must_use]
    pub fn with_schedule(mut self, schedule: ScheduleSpec) -> Self {
        self.schedule = Some(schedule);
        self
    }

    /// Sets the log path.
    #[must_use]
    pub fn with_log(mut self, log_path: impl Into<PathBuf>) -> Self {
        self.log_path = log_path.into();
        self
    }

    /// Validates the profile before exporting it.
    ///
    /// # Errors
    /// Returns [`Error::InvalidArgument`] for empty names, commands, arguments
    /// with NUL bytes or an invalid schedule.
    pub fn validate(&self) -> Result<()> {
        if self.name.trim().is_empty() {
            return Err(Error::InvalidArgument(
                "an automation profile needs a name".to_owned(),
            ));
        }
        if self.command.trim().is_empty() {
            return Err(Error::InvalidArgument(
                "an automation profile needs a command".to_owned(),
            ));
        }
        if self.log_path.as_os_str().is_empty() {
            return Err(Error::InvalidArgument(
                "an automation profile needs a log path".to_owned(),
            ));
        }
        if self.args.iter().any(|argument| argument.contains('\0')) {
            return Err(Error::InvalidArgument(
                "arguments cannot contain NUL bytes".to_owned(),
            ));
        }
        if let Some(schedule) = &self.schedule {
            schedule.validate()?;
        }

        Ok(())
    }

    /// Returns the built-in profiles shipped with FreeSpace.
    #[must_use]
    pub fn builtin_profiles() -> Vec<Self> {
        vec![
            Self::new(
                "daily-scan",
                "freespace.exe",
                vec!["scan".to_owned(), r"C:\".to_owned(), "--json".to_owned()],
            )
            .with_schedule(ScheduleSpec::Daily {
                start: "02:00".to_owned(),
                interval_days: 1,
            })
            .with_log("daily-scan.log"),
            Self::new(
                "weekly-duplicates",
                "freespace.exe",
                vec![
                    "duplicates".to_owned(),
                    r"C:\Users".to_owned(),
                    "--json".to_owned(),
                ],
            )
            .with_schedule(ScheduleSpec::Weekly {
                start: "03:00".to_owned(),
                interval_weeks: 1,
                days: vec![Weekday::Sunday],
            })
            .with_log("weekly-duplicates.log"),
            Self::new(
                "monthly-export",
                "freespace.exe",
                vec![
                    "export".to_owned(),
                    r"C:\".to_owned(),
                    "--format".to_owned(),
                    "html".to_owned(),
                    "--out".to_owned(),
                    "freespace-report.html".to_owned(),
                ],
            )
            .with_schedule(ScheduleSpec::Monthly {
                start: "04:00".to_owned(),
                day: 1,
                interval_months: 1,
            })
            .with_log("monthly-export.log"),
        ]
    }

    /// Looks a built-in profile up by name.
    #[must_use]
    pub fn builtin(name: &str) -> Option<Self> {
        Self::builtin_profiles()
            .into_iter()
            .find(|profile| profile.name.eq_ignore_ascii_case(name))
    }
}

fn validate_start(start: &str) -> Result<()> {
    let Some((hour, minute)) = start.split_once(':') else {
        return Err(Error::InvalidArgument(format!(
            "invalid start time `{start}`; expected `HH:MM`"
        )));
    };
    let hour = hour.parse::<u8>().ok();
    let minute = minute.parse::<u8>().ok();
    if !matches!(hour, Some(0..=23)) || !matches!(minute, Some(0..=59)) {
        return Err(Error::InvalidArgument(format!(
            "invalid start time `{start}`; expected `HH:MM`"
        )));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{AutomationProfile, ScheduleSpec, Weekday};

    #[test]
    fn builtins_are_valid_and_unique() {
        let profiles = AutomationProfile::builtin_profiles();

        assert_eq!(profiles.len(), 3);
        for profile in &profiles {
            profile.validate().expect("valid profile");
        }
        assert!(AutomationProfile::builtin("DAILY-SCAN").is_some());
        assert!(AutomationProfile::builtin("missing").is_none());
    }

    #[test]
    fn validates_schedules() {
        assert!(
            ScheduleSpec::Daily {
                start: "02:00".to_owned(),
                interval_days: 1,
            }
            .validate()
            .is_ok()
        );
        assert!(
            ScheduleSpec::Weekly {
                start: "2:00".to_owned(),
                interval_weeks: 1,
                days: Vec::new(),
            }
            .validate()
            .is_err()
        );
        assert!(
            ScheduleSpec::Monthly {
                start: "24:00".to_owned(),
                day: 1,
                interval_months: 1,
            }
            .validate()
            .is_err()
        );
        assert!(
            ScheduleSpec::Monthly {
                start: "04:00".to_owned(),
                day: 32,
                interval_months: 1,
            }
            .validate()
            .is_err()
        );
    }

    #[test]
    fn parses_weekday_names() {
        assert_eq!(Weekday::parse_value("Sun"), Some(Weekday::Sunday));
        assert_eq!(Weekday::parse_value("воскресенье"), Some(Weekday::Sunday));
        assert_eq!(Weekday::parse_value("holiday"), None);
    }

    #[test]
    fn serialises_profiles() {
        let profile = AutomationProfile::new("scan", "freespace.exe", vec!["scan".into()])
            .with_schedule(ScheduleSpec::AtLogon);
        let json = serde_json::to_string(&profile).expect("serialise");

        assert!(json.contains("\"at-logon\""));
        let decoded: AutomationProfile = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(decoded.name, "scan");
    }
}
