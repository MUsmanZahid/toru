use serde::{Deserialize, Serialize};
use std::{
    env,
    fmt::{self, Display, Formatter},
    fs::File,
    io::{self, Stdin, Stdout, Write},
    num::ParseIntError,
    str::FromStr,
    path::Path,
};

#[derive(Clone, Debug, Serialize, Deserialize)]
enum Tag {
    Pending,
    Complete,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct Task {
    parent: Option<usize>,
    name: String,
    tag: Tag,
    children: Vec<usize>,
}

impl Task {
    fn new() -> Self {
        Task {
            parent: None,
            name: String::from("Default"),
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

    fn from_stdin(
        in_handle: &Stdin,
        out_handle: &mut Stdout,
        parent_idx: usize,
    ) -> Self {
        let mut buffer = String::with_capacity(40);
        write!(out_handle, "Name> ").unwrap();
        out_handle.flush().unwrap();
        in_handle.read_line(&mut buffer).unwrap();

        Task::new()
            .with_name(buffer.trim_end().to_string())
            .set_parent(parent_idx)
    }
}

impl Display for Task {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.name)
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct Tree {
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
        self
            .get_children_of(task)
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

    fn get_children(&self) -> Vec<&Task> {
        let parent = self.get_current();
        let children = &parent.children;
        children
            .iter()
            .map(|&index| self.tasks.get(index).unwrap())
            .collect()
    }

    fn get_children_of(&self, task: &Task) -> Vec<&Task> {
        let children = &task.children;
        children
            .iter()
            .map(|&index| self.tasks.get(index).unwrap())
            .collect()
    }

    fn nth_child(&self, idx: usize) -> Result<usize, ToruError> {
        self.get_current()
            .children
            .get(idx)
            .and_then(|&nth| Some(nth))
            .ok_or(ToruError::InvalidIndex(idx))
    }
}

#[derive(Debug)]
enum ToruError {
    InvalidIndex(usize),
    ParseCommandFailure,
}

impl Display for ToruError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let msg = match self {
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

#[derive(Debug)]
enum Command {
    Add,
    Ascend,
    Complete,
    Delete,
    Descend,
    List,
    Help,
    Exit,
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

const PROMPT: &'static str = "toru> ";

fn main() {
    let file_name = Path::new(".toru.yaml");
    let key = if cfg!(windows) {
        "HOMEPATH"
    } else {
        "HOME"
    };

    let full_path = match env::var(key) {
        Ok(home) => Path::new(&home).join(file_name),
        Err(e) => {
            eprintln!("{}", e);
            return;
        }
    };

    let mut tree = match File::open(&full_path) {
        Ok(file) => serde_yaml::from_reader(file).unwrap(),
        Err(_) => Tree::new(),
    };

    let stdin = io::stdin();
    let mut stdout = io::stdout();
    let mut buffer = String::with_capacity(100);

    loop {
        write!(stdout, "{}", PROMPT).unwrap();
        stdout.flush().unwrap();
        match stdin.read_line(&mut buffer) {
            Ok(0) => break,
            Ok(_) => (),
            Err(e) => {
                eprintln!("{}", e);
                break;
            }
        };

        let cmd = match buffer.trim_end().parse::<Command>() {
            Ok(cmd) => cmd,
            Err(_) => Command::Help,
        };

        tree = match cmd {
            Command::Add => {
                let parent_idx = tree.ptr;
                add(tree, Task::from_stdin(&stdin, &mut stdout, parent_idx))
            }
            Command::Ascend => ascend(tree),
            Command::Complete => {
                verify_index_and(&stdin, &mut stdout, tree, complete)
            }
            Command::Delete => {
                verify_index_and(&stdin, &mut stdout, tree, delete)
            }
            Command::Descend => {
                verify_index_and(&stdin, &mut stdout, tree, descend)
            }
            Command::List => {
                list(&mut stdout, &tree);
                tree
            }
            Command::Help => {
                help(&mut stdout);
                tree
            }
            Command::Exit => {
                break;
            }
        };

        buffer.clear();
    }
    writeln!(stdout, "Saving...").unwrap();
    serde_yaml::to_writer(File::create(&full_path).unwrap(), &tree).unwrap()
}

fn verify_index_and<F>(
    in_handle: &Stdin,
    out_handle: &mut Stdout,
    tree: Tree,
    f: F,
) -> Tree
where
    F: FnOnce(Tree, usize) -> Tree,
{
    match index_from_stdin(in_handle, out_handle) {
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

fn index_from_stdin(
    in_handle: &Stdin,
    out_handle: &mut Stdout,
) -> Result<usize, ParseIntError> {
    let mut buffer = String::with_capacity(4);
    write!(out_handle, "Index> ").unwrap();
    out_handle.flush().unwrap();
    in_handle.read_line(&mut buffer).unwrap();

    buffer.trim_end().parse::<usize>()
}

fn add(mut tree: Tree, task: Task) -> Tree {
    let index_of_child = tree.tasks.len();
    let new_parent = tree.get_current_owned().add_child(index_of_child);
    tree.tasks.push(task);
    tree.replace_current(new_parent)
}

fn ascend(mut tree: Tree) -> Tree {
    if tree.at_root() {
        return tree;
    }

    match tree.get_current().parent {
        Some(parent) => tree.ptr = parent,
        None => unreachable!(),
    }

    tree
}

fn complete(tree: Tree, idx: usize) -> Tree {
    let complete_task = tree.get_task_owned(idx).unwrap().complete();
    tree.replace_task(idx, complete_task)
}

fn delete(mut tree: Tree, idx: usize) -> Tree {
    if idx == 0 {
        return tree;
    }

    let mut parent_idx = tree.get_task(idx).unwrap().parent.unwrap();
    let mut new_parent =
        tree.get_task_owned(parent_idx).unwrap().remove_child(idx);

    tree = tree.replace_task(parent_idx, new_parent);
    tree.tasks.swap_remove(idx);

    // Need to update the parent of the task we just swap_removed to point to
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

fn descend(mut tree: Tree, idx: usize) -> Tree {
    if tree.get_current().is_child(idx) {
        tree.ptr = idx;
    }

    tree
}

fn help(handle: &mut Stdout) {
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

    writeln!(handle, "\nToru help:").unwrap();
    for msg in help {
        writeln!(handle, "{}", msg).unwrap();
    }
    writeln!(handle, "").unwrap();
}

fn list(handle: &mut Stdout, tree: &Tree) {
    let base = if tree.at_root() {
        String::from("Root")
    } else {
        format!("Task - {}", tree.get_current())
    };

    writeln!(handle, "{}", base).unwrap();

    for (id, task) in tree
        .get_children()
        .iter()
        .filter(|task| !task.is_complete())
        .enumerate()
    {
        let subchildren_indicator = if task.has_children() && tree.has_pending(task) {
            format!("+ {}", task)
        } else {
            format!("  {}", task)
        };

        writeln!(handle, "{}. {}", id, subchildren_indicator).unwrap();
    }
}
