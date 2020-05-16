mod cli;
mod task;
mod tree;
mod tui;

use cli::CLI;
use std::{env, error::Error, fmt, fs::File, path::Path};
use tree::Tree;

type Result<T> = std::result::Result<T, Box<dyn Error>>;

fn main() -> Result<()> {
    let file_name = Path::new(".toru.yaml");
    let key = if cfg!(windows) { "HOMEPATH" } else { "HOME" };

    let path = match env::var(key) {
        Ok(home) => Path::new(&home).join(file_name),
        Err(_) => file_name.to_path_buf(),
    };

    let file = File::open(&path);

    let mut tree = if let Ok(file) = file {
        serde_yaml::from_reader::<_, Tree>(file).unwrap()
    } else {
        Tree::new()
    };

    if let Some(value) = env::args().nth(1) {
        if value == "-i" {
            CLI::default().run()?;
        } else if value == "-s" {
            // Server branch
        } else {
            Err(Box::new(ToruError::InstantiateError))?;
        }
    } else if cfg!(windows) {
        CLI::default().run()?;
    } else {
        tree = tui::run(tree)?;
    }

    let file = File::create(&path)?;
    serde_yaml::to_writer(file, &tree)?;

    Ok(())
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
