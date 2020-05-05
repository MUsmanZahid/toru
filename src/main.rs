mod cli;
mod curses;
mod task;
mod tree;

use cli::CLI;
use std::{env, error::Error, fmt};

fn main() -> Result<(), Box<dyn Error>> {
    if let Some(value) = env::args().nth(1) {
        if value == "-i" {
            CLI::default().run()
        } else if value == "-s" {
            // Server branch
            Ok(())
        } else {
            Err(Box::new(ToruError::InstantiateError))
        }
    } else {
        if cfg!(windows) {
            CLI::default().run()
        } else {
            curses::run()
        }
    }
}

#[derive(Debug)]
pub enum ToruError {
    IoError,
    InstantiateError,
    InvalidIndex(usize),
    ParseCommandFailure,
}

impl Error for ToruError {}

impl fmt::Display for ToruError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let msg = match self {
            Self::IoError => String::from("Error in IO operations"),
            Self::InstantiateError => {
                format!("Error creating an instance of toru")
            }
            Self::InvalidIndex(idx) => {
                format!("Child at index {} does not exist", idx)
            }
            Self::ParseCommandFailure => {
                String::from("Failed to parse command")
            }
        };

        write!(f, "{}", msg)
    }
}
