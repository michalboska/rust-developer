use std::error::Error;
use std::fs::read_to_string;

use slug::slugify;

use crate::commands::Command::{Csv, Length, LowerCase, NoSpaces, Reverse, Slugify, UpperCase};
use crate::csv_table::CsvTable;

pub enum Command {
    LowerCase(String),
    UpperCase(String),
    NoSpaces(String),
    Slugify(String),
    Length(String),
    Reverse(String),
    Csv(String),

    // Not a command by itself, tells the other thread to stop receiving any further commands and terminate:
    PoisonPill,
}

impl Command {
    pub fn from_string_and_arg(operation: &str, arg: String) -> Option<Command> {
        match operation {
            "lowercase" => Some(LowerCase(arg)),
            "uppercase" => Some(UpperCase(arg)),
            "no-spaces" => Some(NoSpaces(arg)),
            "slugify" => Some(Slugify(arg)),
            "length" => Some(Length(arg)),
            "reverse" => Some(Reverse(arg)),
            "csv" => Some(Csv(arg)),
            _ => None,
        }
    }

    pub fn execute(&self) -> Result<String, Box<dyn Error>> {
        return match &self {
            LowerCase(str) => Ok(str.to_lowercase()),
            UpperCase(str) => Ok(str.to_uppercase()),
            NoSpaces(str) => Ok(str.replace(" ", "")),
            Slugify(str) => Ok(slugify(str)),
            Length(str) => Ok(str.trim().chars().count().to_string()),
            Reverse(str) => Ok(str.trim().chars().rev().collect()),
            Csv(str) => {
                let csv_content = read_to_string(str)?;
                let result = Ok(CsvTable::from_string(&csv_content)?.to_string());
                result
            }
            Command::PoisonPill => Ok("".to_string()),
        };
    }
}
