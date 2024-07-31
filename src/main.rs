use anyhow::Result;
use chrono::{DateTime, Duration, Local, NaiveTime, Utc, Weekday};
use regex::Regex;
use std::fmt::{Display, Formatter};

use inquire::{required, CustomType, DateSelect, Select, Text};

use serde::{Deserialize, Serialize, de::DeserializeOwned};

mod config;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Activity {
    id: i32,
    parent_title: Option<String>,
    name: String,
}

impl Display for Activity {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), std::fmt::Error> {
        if let Some(parent_title) = &self.parent_title {
            write!(f, "{} | {}", parent_title, self.name)
        } else {
            write!(f, "{}", self.name)
        }
    }
}

#[derive(Debug, Deserialize)]
struct Project {
    id: i32,
    name: String,
}

impl Display for Project {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), std::fmt::Error> {
        write!(f, "{}", self.name)
    }
}

#[derive(Serialize, Debug)]
pub struct TimesheetEditForm {
    pub begin: DateTime<Utc>,
    pub project: i32,
    pub activity: i32,
    pub end: DateTime<Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

fn api_request<T: DeserializeOwned, B: Serialize>(
    config: &config::Config,
    url: &str,
    body: Option<&B>
) -> Result<T> {
    let url = config.endpoint.to_owned() + url;
    let mut request = match body {
        Some(_) => ureq::post(&url),
        None => ureq::get(&url),
    };

    request = request
        .set("Accept", "application/json")
        .set("Authorization", &format!("Bearer {}", config.token));

    let response = match body {
        Some(data) => request.send_json(data),
        None => request.call(),
    };

    let response = match response {
        Ok(response) => {
            Ok(response)
        },
        Err(ureq::Error::Status(code, response)) => {
            /* the server returned an unexpected status
               code (such as 400, 500 etc) */
            Err(anyhow::anyhow!("Server returned status code {}: {}", code, response.into_string().unwrap()))
        }
        Err(e) => {
            anyhow::bail!("Request failed: {:?}", e)
        }
    }?;

    let data: T = serde_json::from_str(&response.into_string().unwrap())?;
    Ok(data)
}

fn get_projects(config: &config::Config) -> Result<Vec<Project>> {
    api_request(
        config,
        "/api/projects?visible=1",
        None::<&()>
    )
}

fn get_activities_by_project(config: &config::Config, project_id: i32) -> Result<Vec<Activity>> {
    let url = format!(
        "/api/activities?visible=1&projects[]={}",
        project_id
    );
    api_request(config, &url, None::<&()>)
}

fn insert_timesheet_entry(config: &config::Config, form: TimesheetEditForm) -> Result<()> {
    api_request(config, "/api/timesheets", Some(&form))
}

fn parse_duration(input: &str) -> Option<Duration> {
    let decimal_re = Regex::new(r"^\d+(\.\d+)?$").unwrap();
    let time_re = Regex::new(r"^(\d+):(\d+)$").unwrap();

    if decimal_re.is_match(input) {
        // Parse as decimal hours
        let hours: f64 = input.parse().ok()?;
        Some(Duration::seconds((hours * 3600.0) as i64))
    } else if let Some(captures) = time_re.captures(input) {
        // Parse as HH:MM format
        let hours: i64 = captures.get(1)?.as_str().parse().ok()?;
        let minutes: i64 = captures.get(2)?.as_str().parse().ok()?;
        Some(Duration::hours(hours) + Duration::minutes(minutes))
    } else {
        None
    }
}

fn main() -> Result<()> {
    // TODO Inform user errors from this are coming from loading the config
    let config = config::load_config()?;

    let projects = get_projects(&config)?;

    let proj = Select::new("Project:", projects).prompt()?;

    let activity =
        Select::new("Activity:", get_activities_by_project(&config, proj.id)?).prompt()?;

    let duration = Text::new("Duration:")
        .with_validator(required!("This field is required"))
        .with_help_message("E.g. 1.5 or 2:30")
        .with_default("1")
        .prompt()?;

    let duration = parse_duration(&duration).unwrap();

    let date = DateSelect::new("Date:")
        .with_week_start(Weekday::Mon)
        .prompt()?;

    // If the selected date is the current date, default to current time minus duration.
    // Otherwise, use the configured value.
    let default_time = if date == Local::now().date_naive() {
        Local::now().time() - duration
    } else {
        config.default_start_time
    };

    let time_prompt = CustomType::<NaiveTime>::new("Enter start time (HH:MM):")
        .with_default_value_formatter(&|t| t.format("%H:%M").to_string())
        .with_error_message("Please enter a valid time in HH:MM format")
        .with_default(default_time)
        .with_help_message("Enter the time in 24-hour format (e.g., 14:30 for 2:30 PM)")
        .prompt()?;

    let description = Text::new("Description:")
        .with_help_message("optional")
        .prompt()?;

    let begin = date
        .and_time(time_prompt)
        .and_local_timezone(Local)
        .earliest()
        .unwrap();

    let end = begin + duration;

    insert_timesheet_entry(
        &config,
        TimesheetEditForm {
            begin: begin.into(),
            project: proj.id,
            activity: activity.id,
            end: end.into(),
            description: Some(description),
        },
    )?;

    Ok(())
}
