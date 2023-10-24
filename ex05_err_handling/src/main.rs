use std::{env, io};
use std::error::Error;
use std::io::BufRead;
use std::process::exit;

use slug::slugify;

use csv_table::CsvTable;
use err::ArgParseError;

mod csv_table;
mod err;

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Missing command line argument!");
        exit(1);
    }
    println!("Input:");
    match get_transformed_string(&args[1]) {
        Ok(result) => {
            println!("Result: \n{}", result);
        }
        Err(e) => {
            eprintln!("{}", e.to_string());
            exit(1);
        }
    }
}

fn get_transformed_string(string: &String) -> Result<String, Box<dyn Error>> {
    let transform_fn = get_transform_fn(string)?;
    let mut buffer: String = String::from("");
    {
        for line in io::stdin().lock().lines() {
            buffer.push_str(&line?);
            buffer.push('\n');
        }
    }
    return transform_fn(&buffer);
}

fn get_transform_fn(operation: &String) -> Result<fn(&str) -> Result<String, Box<dyn Error>>, Box<dyn Error>> {
    return match operation.as_str() {
        "lowercase" => Ok(|s| { Ok(s.to_lowercase()) }),
        "uppercase" => Ok(|s| { Ok(s.to_uppercase()) }),
        "no-spaces" => Ok(|s| { Ok(s.replace(" ", "")) }),
        "slugify" => Ok(|s| { Ok(slugify(s)) }),
        "length" => Ok(|s| { Ok(s.trim().chars().count().to_string()) }),
        "reverse" => Ok(|s| { Ok(s.trim().chars().rev().collect()) }),
        "csv" => Ok(|s| { Ok(CsvTable::from_string(s)?.to_string()) }),
        _ => Err(Box::new(ArgParseError {
            msg: String::from("Command line argument must be one of: lowercase,uppercase,no-spaces,slugify,length,reverse,csv")
        })),
    };
}