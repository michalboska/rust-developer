use std::{io, thread};
use std::env::args;
use std::error::Error;
use std::io::BufRead;
use std::process::exit;

use flume::{Receiver, Sender};

use crate::commands::Command;
use crate::commands::Command::PoisonPill;
use crate::err::ThreadingError;

mod commands;
mod csv_table;
mod err;

const ERROR_INVALID_COMMAND: &str = "<command> must be one of: lowercase,uppercase,no-spaces,slugify,length,reverse,csv";

fn main() {
    let args_vec = args().collect::<Vec<String>>();
    let result_fn = || {
        return if args_vec.len() < 2 {
            interactive_mode()
        } else {
            immediate_mode(&args_vec[1])
        };
    };
    match result_fn() {
        Ok(_) => {}
        Err(err) => {
            eprintln!("{}", err);
            exit(1);
        }
    }
}

fn immediate_mode(command_str: &str) -> Result<(), Box<dyn Error>> {
    println!("Input:");
    let mut buf = String::new();
    io::stdin().read_line(&mut buf)?;
    let command = Command::from_string_and_arg(command_str, buf.trim().to_string()).ok_or(err::ArgParseError { msg: ERROR_INVALID_COMMAND.to_string() })?;
    let result_str = command.execute()?;
    println!("{}", result_str);
    return Ok(());
}

fn interactive_mode() -> Result<(), Box<dyn Error>> {
    let (tx, rx) = flume::unbounded();
    let join_handle = thread::spawn(move || {
        input_parser(tx);
    });
    thread::spawn(move || {
        command_processor(rx);
    });

    return Ok(join_handle.join().map_err(|_| { Box::new(ThreadingError {}) })?);
}

fn input_parser(tx: Sender<Command>) {
    for line_res in io::stdin().lock().lines() {
        let line = line_res.unwrap();
        let command_with_input = line.splitn(2, " ").collect::<Vec<&str>>();
        if command_with_input.len() != 2 {
            eprintln!("Usage: <command> <input>");
            continue;
        }
        let command_str = &command_with_input[0];
        let arg_str = &command_with_input[1];
        let parse_result = Command::from_string_and_arg(command_str, arg_str.to_string());
        match parse_result {
            Some(command) => {
                tx.send(command).unwrap();
            }
            None => {
                eprintln!("{}", ERROR_INVALID_COMMAND);
            }
        }
    }
    tx.send(PoisonPill).unwrap();
}

fn command_processor(rx: Receiver<Command>) {
    loop {
        let recv_result = rx.recv();
        match recv_result {
            Ok(command) => {
                if matches!(command, PoisonPill) {
                    break;
                } else {
                    match command.execute() {
                        Ok(result_str) => {
                            println!("{}", result_str);
                        }
                        Err(err) => {
                            eprintln!("Error processing input: {}", err)
                        }
                    }
                }
            }
            Err(err) => {
                eprintln!("Receive error: {}", err);
                break;
            }
        }
    }
}
