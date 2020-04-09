use serde::{Deserialize, Serialize};
use std::{
    cmp::PartialEq,
    env,
    fmt::{self, Display, Formatter},
    fs::File,
    io::{self, Write},
    net::TcpListener,
    num::ParseIntError,
    path::{Path, PathBuf},
    str::FromStr,
    time::SystemTime,
};
use time::PrimitiveDateTime;

#[derive(Debug)]
pub enum ToruError {
    InstantiateError,
    InvalidIndex(usize),
    ParseCommandFailure,
}

impl Display for ToruError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let msg = match self {
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

#[derive(Clone, Debug, Serialize, Deserialize)]
enum Tag {
    Pending,
    Complete,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Task {
    parent: Option<usize>,
    name: String,
    due: Option<PrimitiveDateTime>,
    tag: Tag,
    children: Vec<usize>,
}

impl Task {
    fn new() -> Self {
        Task {
            parent: None,
            name: String::from("Default"),
            due: None,
            tag: Tag::Pending,
            children: Vec::new(),
        }
    }

    fn set_parent(mut self, parent_index: usize) -> Self {
        self.parent = Some(parent_index);
        self
    }

    fn with_name(mut self, name: String) -> Self {
        self.name = name;
        self
    }

    fn due_at(mut self, date: PrimitiveDateTime) -> Self {
        self.due = Some(date);
        self
    }

    fn complete(mut self) -> Task {
        self.tag = Tag::Complete;
        self
    }

    fn is_complete(&self) -> bool {
        match self.tag {
            Tag::Complete => true,
            _ => false,
        }
    }

    fn add_child(mut self, child_index: usize) -> Self {
        self.children.push(child_index);
        self
    }

    fn remove_child(mut self, child_index: usize) -> Self {
        self.children.retain(|&index| index != child_index);
        self
    }

    fn replace_child(mut self, old_child: usize, new_child: usize) -> Self {
        let new_children: Vec<usize> = self
            .children
            .iter()
            .map(|&index| if index == old_child { new_child } else { index })
            .collect();

        self.children = new_children;
        self
    }

    fn has_children(&self) -> bool {
        !self.children.is_empty()
    }

    fn is_child(&self, id: usize) -> bool {
        self.children.contains(&id)
    }

    pub fn from_stdin(
        io: &mut IO,
        parent_idx: usize,
    ) -> Result<Self, time::ParseError> {
        let date_format = "%F %I:%M %p";
        let now = PrimitiveDateTime::from(SystemTime::now());

        io.write(String::from("Name> "));
        let name = io.readln();

        io.write(format!("Due [{}]", now.format(date_format)));
        let date = io.readln();

        let date = date.trim_end();

        let task = Task::new()
            .with_name(name.trim_end().to_string())
            .set_parent(parent_idx);
        Ok(if date.is_empty() {
            task
        } else {
            task.due_at(time::parse(date, date_format)?)
        })
    }
}

impl Display for Task {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self.due {
            Some(due) => {
                write!(f, "{} | {}", self.name, due.format("%I:%M %p %F"))
            }
            None => write!(f, "{}", self.name),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Tree {
    ptr: usize,
    tasks: Vec<Task>,
}

impl Tree {
    fn new() -> Self {
        Self {
            ptr: 0,
            tasks: vec![Task::new()],
        }
    }

    fn ptr(&self) -> usize {
        self.ptr
    }

    fn at_root(&self) -> bool {
        self.ptr == 0
    }

    fn get_current(&self) -> &Task {
        &self.tasks[self.ptr]
    }

    fn get_current_owned(&self) -> Task {
        self.tasks[self.ptr].clone()
    }

    fn get_task(&self, id: usize) -> Option<&Task> {
        self.tasks.get(id)
    }

    fn get_task_owned(&self, id: usize) -> Option<Task> {
        self.tasks.get(id).and_then(|task| Some(task.clone()))
    }

    fn has_pending(&self, task: &Task) -> bool {
        self.children_of(task)
            .iter()
            .any(|child| !child.is_complete())
    }

    fn replace_current(mut self, new_task: Task) -> Self {
        self.tasks[self.ptr] = new_task;
        self
    }

    fn replace_task(mut self, task_id: usize, new_task: Task) -> Self {
        self.tasks[task_id] = new_task;
        self
    }

    fn children(&self) -> Vec<&Task> {
        let parent = self.get_current();
        let children = &parent.children;
        children
            .iter()
            .map(|&index| self.tasks.get(index).unwrap())
            .collect()
    }

    fn children_of(&self, task: &Task) -> Vec<&Task> {
        let children = &task.children;
        children
            .iter()
            .map(|&index| self.tasks.get(index).unwrap())
            .collect()
    }

    fn pending_children(&self) -> Vec<&Task> {
        self.children()
            .iter()
            .filter(|&child| !child.is_complete())
            .map(|&task| task)
            .collect()
    }

    fn nth_child(&self, idx: usize) -> Result<usize, ToruError> {
        self.get_current()
            .children
            .iter()
            .filter(|&&task_idx| {
                let task = self.get_task(task_idx).unwrap();
                !task.is_complete()
            })
            .nth(idx)
            .and_then(|&nth| Some(nth))
            .ok_or(ToruError::InvalidIndex(idx))
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
                match Task::from_stdin(io, parent_idx) {
                    Ok(task) => add(tree, task),
                    Err(e) => {
                        eprintln!("{}", e);
                        tree
                    }
                }
            }
            Self::Ascend => ascend(tree),
            Self::Complete => verify_index_and(io, tree, complete),
            Self::Delete => verify_index_and(io, tree, delete),
            Self::Descend => verify_index_and(io, tree, descend),
            Self::List => {
                list(io, &tree);
                tree
            }
            Command::Help => {
                help(io);
                tree
            }
            Command::Exit => unreachable!(),
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

pub struct Config {
    save_file: PathBuf,
}

impl Config {
    fn new(save_file: PathBuf) -> Self {
        Config { save_file }
    }

    pub fn load(&self) -> Tree {
        let file_status = File::open(&self.save_file);

        if file_status.is_ok() {
            let file = file_status.unwrap();
            serde_yaml::from_reader(file).unwrap()
        } else {
            Tree::new()
        }
    }

    fn unload(&self, tree: Tree) {
        println!("Saving...");
        match File::create(&self.save_file) {
            Ok(file) => serde_yaml::to_writer(file, &tree).unwrap(),
            Err(e) => {
                eprintln!("{}", e);
            }
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        let file_name = Path::new(".toru.yaml");
        let key = if cfg!(windows) { "HOMEPATH" } else { "HOME" };

        let full_path = match env::var(key) {
            Ok(home) => Path::new(&home).join(file_name),
            Err(_) => file_name.to_path_buf(),
        };

        Config::new(full_path)
    }
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
    io.write(String::from("Index> "));
    io.readln().trim_end().parse::<usize>()
}

pub fn add(mut tree: Tree, task: Task) -> Tree {
    let index_of_child = tree.tasks.len();
    let new_parent = tree.get_current_owned().add_child(index_of_child);
    tree.tasks.push(task);
    tree.replace_current(new_parent)
}

pub fn ascend(mut tree: Tree) -> Tree {
    if tree.at_root() {
        return tree;
    }

    match tree.get_current().parent {
        Some(parent) => tree.ptr = parent,
        None => unreachable!(),
    }

    tree
}

pub fn complete(tree: Tree, idx: usize) -> Tree {
    let complete_task = tree.get_task_owned(idx).unwrap().complete();
    tree.replace_task(idx, complete_task)
}

pub fn delete(mut tree: Tree, idx: usize) -> Tree {
    if idx == 0 {
        return tree;
    }

    let mut parent_idx = tree.get_task(idx).unwrap().parent.unwrap();
    let mut new_parent =
        tree.get_task_owned(parent_idx).unwrap().remove_child(idx);

    tree = tree.replace_task(parent_idx, new_parent);
    tree.tasks.swap_remove(idx);

    // Need to update the parent of the task we just swap removed to point to
    // the correct index (the index we just deleted)
    let idx_to_replace = tree.tasks.len();
    parent_idx = match tree.get_task(idx) {
        Some(id) => id.parent.unwrap(),
        // If there is nothing at the index we just deleted then we were
        // deleting the last node in the tree
        None => return tree,
    };

    new_parent = tree
        .get_task_owned(parent_idx)
        .unwrap()
        .replace_child(idx_to_replace, idx);
    tree.replace_task(parent_idx, new_parent)
}

pub fn descend(mut tree: Tree, idx: usize) -> Tree {
    if tree.get_current().is_child(idx) {
        tree.ptr = idx;
    }

    tree
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
        (tree.get_current().name.as_str(), "\u{2191}")
    };

    io.writeln(format!(
        "{}\n{}\n{:-<underline$}",
        parent_indicator,
        label,
        "",
        underline = label.len()
    ));

    for (id, task) in tree.pending_children().iter().enumerate() {
        let subchildren_indicator =
            if task.has_children() && tree.has_pending(task) {
                format!("+ {}", task)
            } else {
                format!("  {}", task)
            };

        io.writeln(format!("{}. {}", id, subchildren_indicator));
    }
    io.writeln(String::from(""));
}

pub struct Instance {
    run_as: Box<dyn Runable>,
    config: Config,
}

impl Instance {
    fn new(run_as: Box<dyn Runable>) -> Self {
        let config: Config = Default::default();
        Self { run_as, config }
    }

    pub fn from_args() -> Result<Self, ToruError> {
        let args: Option<String> = env::args().nth(1);

        if args.is_some() {
            let value = args.unwrap();
            if value == "-i" {
                Ok(Instance::new(Box::new(Interactive::new())))
            } else if value == "-s" {
                Ok(Instance::new(Box::new(Server::new())))
            } else {
                Err(ToruError::InstantiateError)
            }
        } else {
            Ok(Instance::new(Box::new(Interactive::new())))
        }
    }

    pub fn run(mut self) {
        self.run_as.run(self.config)
    }
}

trait Runable {
    fn run(&mut self, config: Config);
}

struct Server;

impl Server {
    fn new() -> Self {
        Server
    }
}

impl Runable for Server {
    fn run(&mut self, _config: Config) {}
}

struct Interactive {
    prompt: &'static str,
    io: IO,
}

impl Interactive {
    fn new() -> Self {
        Self {
            prompt: "toru> ",
            io: IO::new(),
        }
    }
}

impl Runable for Interactive {
    fn run(&mut self, config: Config) {
        let mut tree = config.load();
        loop {
            self.io.write(format!("{}", self.prompt));
            match io::stdin().read_line(&mut self.io.buffer) {
                Ok(0) => break,
                Err(e) => {
                    eprintln!("{}", e);
                    break;
                }
                _ => (),
            };

            let cmd = match self.io.buffer.trim_end().parse::<Command>() {
                Ok(cmd) => cmd,
                Err(_) => Command::Help,
            };

            if cmd == Command::Exit {
                break;
            }

            tree = cmd.run(&mut self.io, tree);
            self.io.buffer.clear();
        }

        config.unload(tree);
    }
}

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

    fn readln(&mut self) -> String {
        self.buffer.clear();
        self.stdin.read_line(&mut self.buffer).unwrap();
        self.buffer.clone()
    }

    fn writeln(&mut self, s: String) {
        writeln!(self.stdout, "{}", s).unwrap()
    }

    fn write(&mut self, s: String) {
        write!(self.stdout, "{}", s).unwrap();
        self.stdout.flush().unwrap();
    }
}
