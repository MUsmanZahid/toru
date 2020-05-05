use std::env;

use crate::task::Task;
use crate::tree::{self, Tree};
use crate::ToruError;

use std::{
    error::Error,
    fmt,
    fs::File,
    io::{self, Write},
    num::ParseIntError,
    path::{Path, PathBuf},
    str::FromStr,
    time::SystemTime,
};
use time::PrimitiveDateTime;

pub struct IO {
    stdin: io::Stdin,
    stdout: io::Stdout,
    buffer: String,
}

impl IO {
    pub fn new() -> Self {
        IO {
            stdin: io::stdin(),
            stdout: io::stdout(),
            buffer: String::with_capacity(40),
        }
    }

    pub fn readln(&mut self) -> String {
        self.buffer.clear();
        self.stdin.read_line(&mut self.buffer).unwrap();
        self.buffer.clone()
    }

    pub fn writeln<S>(&mut self, s: S)
    where
        S: AsRef<str> + fmt::Display,
    {
        writeln!(self.stdout, "{}", s).unwrap()
    }

    pub fn write<S>(&mut self, s: S)
    where
        S: AsRef<str> + fmt::Display,
    {
        write!(self.stdout, "{}", s).unwrap();
        self.stdout.flush().unwrap();
    }
}

#[derive(Debug, PartialEq)]
pub enum Command {
    Add,
    Ascend,
    Complete,
    Delete,
    Descend,
    List,
    Help,
    Exit,
}

impl Command {
    fn run(self, io: &mut IO, tree: Tree) -> Tree {
        match self {
            Self::Add => {
                let parent_idx = tree.ptr();
                match task_from_stdin(io, parent_idx) {
                    Ok(task) => tree::add(tree, task),
                    Err(e) => {
                        eprintln!("{}", e);
                        tree
                    }
                }
            }
            Self::Ascend => tree::ascend(tree),
            Self::Complete => verify_index_and(io, tree, tree::complete),
            Self::Delete => verify_index_and(io, tree, tree::delete),
            Self::Descend => verify_index_and(io, tree, tree::descend),
            Self::List => {
                list(io, &tree);
                tree
            }
            Self::Help => {
                help(io);
                tree
            }
            Self::Exit => unreachable!(),
        }
    }
}

impl FromStr for Command {
    type Err = ToruError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "add" => Ok(Self::Add),
            "up" => Ok(Self::Ascend),
            "del" => Ok(Self::Delete),
            "done" => Ok(Self::Complete),
            "down" => Ok(Self::Descend),
            "list" => Ok(Self::List),
            "help" => Ok(Self::Help),
            "exit" => Ok(Self::Exit),
            _ => Err(Self::Err::ParseCommandFailure),
        }
    }
}

pub struct CLI {
    save_path: PathBuf,
    io: IO,
    tree: Tree,
}

impl CLI {
    pub fn new(save_path: PathBuf, tree: Tree) -> Self {
        Self {
            save_path,
            io: IO::new(),
            tree,
        }
    }

    pub fn run(mut self) -> Result<(), Box<dyn Error>> {
        loop {
            self.io.write("toru> ");
            self.io.buffer.clear();

            let bytes_read = io::stdin()
                .read_line(&mut self.io.buffer)?;

            if bytes_read == 0 {
                break;
            }

            let cmd = match self.io.buffer.trim_end().parse::<Command>() {
                Ok(cmd) => cmd,
                Err(_) => Command::Help,
            };
            self.io.buffer.clear();

            if cmd == Command::Exit {
                break;
            }

            self.tree = cmd.run(&mut self.io, self.tree);
        }

        self.io.writeln("Saving...");
        match File::create(&self.save_path) {
            Ok(file) => serde_yaml::to_writer(file, &self.tree).unwrap(),
            Err(e) => {
                eprintln!("{}", e);
            }
        }

        Ok(())
    }
}

impl Default for CLI {
    fn default() -> Self {
        let file_name = Path::new(".toru.yaml");
        let key = if cfg!(windows) { "HOMEPATH" } else { "HOME" };

        let path = match env::var(key) {
            Ok(home) => Path::new(&home).join(file_name),
            Err(_) => file_name.to_path_buf(),
        };

        let file = File::open(&path);

        let tree = if file.is_ok() {
            let file = file.unwrap();
            serde_yaml::from_reader::<_, Tree>(file).unwrap()
        } else {
            Tree::new()
        };

        Self::new(path, tree)
    }
}

pub fn help(io: &mut IO) {
    let help = vec![
        "add - Add a task.",
        "delete - Delete a task.",
        "done - Complete a task.",
        "down - Traverse 'down' into a task.",
        "exit - Exit toru.",
        "help - Show the help message.",
        "list - Print current task and its children",
        "up - Traverse 'up' to a tasks' parent",
    ];

    io.writeln(String::from("\nToru help:"));
    for msg in help {
        io.writeln(format!("{}", msg));
    }
    io.writeln(String::from(""));
}

pub fn list(io: &mut IO, tree: &Tree) {
    let (label, parent_indicator) = if tree.at_root() {
        ("Home", "")
    } else {
        (tree.current().name().as_str(), "\u{2191}")
    };

    io.writeln(format!(
        "{}\n{}\n{:-<underline$}",
        parent_indicator,
        label,
        "",
        underline = label.len()
    ));

    for (id, task) in tree.pending_children().enumerate() {
        let subchildren_indicator =
            if task.has_children() && tree.has_pending(task) {
                format!("+ {}", task)
            } else {
                format!("  {}", task)
            };

        io.writeln(format!("{}. {}", id + 1, subchildren_indicator));
    }
    io.writeln(String::from(""));
}

pub fn task_from_stdin(
    io: &mut IO,
    parent_idx: usize,
) -> Result<Task, time::ParseError> {
    let date_format = "%F %I:%M %p";
    let now = PrimitiveDateTime::from(SystemTime::now());

    io.write(String::from("Name> "));
    let name = io.readln();

    io.write(format!("Due [{}]", now.format(date_format)));
    let date = io.readln();

    let date = date.trim_end();

    let task = Task::new()
        .set_name(name.trim_end().to_string())
        .set_parent(parent_idx);
    Ok(if date.is_empty() {
        task
    } else {
        task.set_due(time::parse(date, date_format)?)
    })
}

pub fn verify_index_and<F>(io: &mut IO, tree: Tree, f: F) -> Tree
where
    F: FnOnce(Tree, usize) -> Tree,
{
    match index_from_stdin(io) {
        Ok(idx) => match tree.nth_child(idx) {
            Ok(nth) => f(tree, nth),
            Err(e) => {
                eprintln!("{}", e);
                tree
            }
        },
        Err(e) => {
            eprintln!("{}", e);
            tree
        }
    }
}

fn index_from_stdin(io: &mut IO) -> Result<usize, ParseIntError> {
    io.write("Index> ");
    io.readln().trim_end().parse::<usize>().and_then(|idx| Ok(idx - 1))
}
