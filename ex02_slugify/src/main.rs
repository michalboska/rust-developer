use std::{env, io};
use std::process::exit;

use slug::slugify;

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        println!("Missing command line argument!");
        exit(1);
    }
    let transform_fn = get_transform_fn(String::from(&args[1]));
    let mut buffer: String = String::from("");
    let _ = io::stdin().read_line(&mut buffer);
    println!("Result: {}", transform_fn(buffer));
}

fn get_transform_fn(operation: String) -> fn(String) -> String {
    return match operation.as_str() {
        "lowercase" => |s| { s.to_lowercase() },
        "uppercase" => |s| { s.to_uppercase() },
        "no-spaces" => |s| { s.replace(" ", "") },
        "slugify" => |s| { slugify(s) },
        "length" => |s| { s.trim().chars().count().to_string() },
        "reverse" => |s| { s.trim().chars().rev().collect() },
        _ => {
            println!("Command line argument must be one of: lowercase,uppercase,no-spaces,slugify,length,reverse");
            exit(1);
        }
    };
}