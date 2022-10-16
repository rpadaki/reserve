use chrono::{Datelike, NaiveDate, NaiveDateTime, NaiveTime, Weekday};
use clap::Parser;
use serde::{Deserialize, Serialize};
use serde_aux::container_attributes::deserialize_struct_case_insensitive;
use serde_json::{json, Value};
use std::fmt::Debug;

fn get_next_occurrence(
    today: &NaiveDate,
    day_str: &str,
    time_str: &str,
) -> Result<NaiveDateTime, String> {
    let day = match day_str.parse::<Weekday>() {
        Ok(day) => day,
        Err(_) => return Err(format!("Invalid day: {}", day_str)),
    };
    let time = match NaiveTime::parse_from_str(time_str, "%I:%M %p") {
        Ok(time) => time,
        Err(_) => return Err(format!("Invalid time: {}", time_str)),
    };
    let mut next_occurrence = today.and_time(time);
    while next_occurrence.weekday() != day {
        next_occurrence = next_occurrence + chrono::Duration::days(1);
    }
    Ok(next_occurrence)
}

fn is_tomorrow(today: &NaiveDate, dt: &NaiveDateTime) -> bool {
    let tomorrow = *today + chrono::Duration::days(1);
    dt.date() == tomorrow
}

fn split_name(name: &str) -> Result<(&str, &str), String> {
    let mut split = name.splitn(2, ' ');
    let first = match split.next() {
        Some(first) => first,
        None => "",
    };
    if first.len() == 0 {
        return Err(format!("Invalid name: '{}'", name));
    }
    let last = match split.next() {
        Some(last) => last,
        None => "",
    };
    Ok((first, last))
}

fn validate_email(email: &str) -> Result<(), String> {
    if email.is_empty() {
        return Err("Email is required".to_string());
    }
    if !email.contains('@') {
        return Err("Email must contain an @".to_string());
    }
    let mut split = email.splitn(2, '@');
    match split.next() {
        Some(local) => local,
        None => return Err(format!("Invalid email: '{}'", email)),
    };
    let domain = match split.next() {
        Some(domain) => domain,
        None => return Err(format!("Invalid email: '{}'", email)),
    };
    if !domain.contains('.') {
        return Err("Email domain must contain a .".to_string());
    }
    split = domain.splitn(2, '.');
    match split.next() {
        Some(part) => {
            if part.len() == 0 {
                return Err(format!("Invalid email: '{}'", email));
            }
        }
        None => return Err(format!("Invalid domain: '{}'", domain)),
    };
    match split.next() {
        Some(part) => {
            if part.len() == 0 {
                return Err(format!("Invalid email: '{}'", email));
            }
        }
        None => return Err(format!("Invalid domain: '{}'", domain)),
    };
    Ok(())
}

fn standardize_phone(phone: &str) -> Result<String, String> {
    let digits = phone
        .chars()
        .filter(|c| c.is_ascii_digit())
        .collect::<String>();
    if digits.len() != 10 {
        return Err(format!("Invalid phone number: '{}'", phone));
    }
    let mut standardized = String::new();
    for (i, digit) in digits.chars().enumerate() {
        if i == 0 {
            standardized.push('(');
        } else if i == 3 {
            standardized.push_str(") ");
        } else if i == 6 {
            standardized.push('-');
        }
        standardized.push(digit);
    }
    Ok(standardized)
}

fn validate_guests(guests: u8) -> Result<(), String> {
    if guests == 0 {
        return Err("Guests must be greater than 0".to_string());
    }
    if guests > 10 {
        return Err(
            "Ambitious, are we? Try doing this manually for more than 10 guests.".to_string(),
        );
    }
    Ok(())
}

fn create_body(args: &Cli) -> Result<Value, String> {
    let (first, last) = split_name(&args.name)?;
    validate_email(&args.email)?;
    let today = chrono::Local::today().naive_local();
    let next_occurrence = get_next_occurrence(&today, &args.day, &args.time)?;
    Ok(json!({
        "form_category": "Reservation",
        "form_fields": {
            "number_of_guests": validate_guests(args.guests)?,
            "first_name": first,
            "last_name": last,
            "email": &args.email,
            "phone": standardize_phone(&args.phone)?,
            "year": next_occurrence.year(),
            "month": next_occurrence.month(),
            "date": next_occurrence.day(),
            "time": {
                "time": next_occurrence.time().format("%I:%M %p").to_string(),
                "next_day": is_tomorrow(&today, &next_occurrence),
            },
            "space": "Indoors", // TODO: Make this configurable
            "instructions": &args.instructions,
            "texting_permission": false, // TODO: make this configurable
        }
    }))
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
enum SpothopperRestaurants {
    #[serde(deserialize_with = "deserialize_struct_case_insensitive")]
    Slainte = 0xBAE,
}

#[derive(Parser)]
struct Cli {
    #[clap(short, long)]
    name: String,
    #[clap(short, long, default_value = "2")]
    guests: u8,
    #[clap(short, long)]
    email: String,
    #[clap(short, long)]
    phone: String,
    #[clap(short, long, default_value = "Friday")]
    day: String,
    #[clap(short, long, default_value = "7:00 PM")]
    time: String,
    #[clap(short, long)]
    instructions: Option<String>,
}

fn make_spothopper_request_url(restaurant: SpothopperRestaurants) -> String {
    format!(
        "https://www.spothopperapp.com/api/spots/{}/reservation_requests/add_from_tmt",
        restaurant as u16
    )
}

fn main() -> Result<(), String> {
    let args = Cli::parse();
    let body = create_body(&args)?;
    let client = reqwest::blocking::Client::new();
    let response = client
        .post(&make_spothopper_request_url(SpothopperRestaurants::Slainte))
        .header("Content-Type", "application/x-www-form-urlencoded")
        .json(&body)
        .send()
        .map_err(|e| e.to_string())?;
    if response.status().is_success() {
        println!("Successfully made reservation!");
        Ok(())
    } else {
        Err(format!(
            "Failed to make reservation: {}",
            response.text().unwrap()
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_next_occurrence() {
        let today = NaiveDate::from_ymd(2021, 10, 12);
        let next_occurrence = get_next_occurrence(&today, "wednesday", "7:30pm").unwrap();
        assert_eq!(next_occurrence.date(), NaiveDate::from_ymd(2021, 10, 13));
        assert_eq!(next_occurrence.time(), NaiveTime::from_hms(19, 30, 0));

        let next_occurrence = get_next_occurrence(&today, "saturday", "4:20 AM").unwrap();
        assert_eq!(next_occurrence.date(), NaiveDate::from_ymd(2021, 10, 16));
        assert_eq!(next_occurrence.time(), NaiveTime::from_hms(4, 20, 0));

        let next_occurrence = get_next_occurrence(&today, "sunday", "12:00am").unwrap();
        assert_eq!(next_occurrence.date(), NaiveDate::from_ymd(2021, 10, 17));
        assert_eq!(next_occurrence.time(), NaiveTime::from_hms(0, 0, 0));
    }

    #[test]
    fn test_is_tomorrow() {
        let today = NaiveDate::from_ymd(2021, 10, 12);
        let tomorrow = NaiveDate::from_ymd(2021, 10, 13).and_hms(0, 0, 0);
        let next_week = NaiveDate::from_ymd(2021, 10, 19).and_hms(0, 0, 0);
        assert_eq!(is_tomorrow(&today, &tomorrow), true);
        assert_eq!(is_tomorrow(&today, &next_week), false);
    }

    #[test]
    fn test_split_name() {
        assert_eq!(split_name("Jane Smith").unwrap(), ("Jane", "Smith"));
        assert_eq!(split_name("Richard").unwrap(), ("Richard", ""));
        assert_eq!(split_name("Richard ").unwrap(), ("Richard", ""));
        assert!(split_name("").is_err());
    }

    #[test]
    fn test_validate_email() {
        assert!(validate_email("janesmith@provider.net").is_ok());
        assert!(validate_email("janesmith+email@provider.net").is_ok());
        assert!(validate_email("janesmith+email@provider").is_err());
        assert!(validate_email("janesmith+email@provider.").is_err());
        assert!(validate_email("janesmith+email@provider").is_err());
        assert!(validate_email("janesmith+email@.net").is_err());
    }

    #[test]
    fn test_standardize_phone() {
        assert_eq!(standardize_phone("800-867-5309").unwrap(), "(800) 867-5309");
        assert_eq!(standardize_phone("8008675309").unwrap(), "(800) 867-5309");
        assert_eq!(standardize_phone("800 867 5309").unwrap(), "(800) 867-5309");
        assert_eq!(standardize_phone("800.867.5309").unwrap(), "(800) 867-5309");
        assert!(standardize_phone("800867530").is_err());
        assert!(standardize_phone("80086753099").is_err());
        assert!(standardize_phone("8008675309 ext 3203").is_err());
    }

    #[test]
    fn test_validate_guests() {
        assert!(validate_guests(1).is_ok());
        assert!(validate_guests(10).is_ok());
        assert!(validate_guests(0).is_err());
        assert!(validate_guests(11).is_err());
    }

    #[test]
    fn test_slainte_url() {
        assert_eq!(
            make_spothopper_request_url(SpothopperRestaurants::Slainte),
            "https://www.spothopperapp.com/api/spots/2990/reservation_requests/add_from_tmt"
        );
    }
}
