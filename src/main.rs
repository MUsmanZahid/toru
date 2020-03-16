use clap::{self, load_yaml, App, ArgMatches};
use serde::{Deserialize, Serialize};
use std::{
    fmt::{self, Display, Formatter},
    fs::File,
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

    fn is_child(&self, id: usize) -> bool {
        self.children.contains(&id)
    }
}

impl Display for Task {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.name)
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct State {
    ptr: usize,
    tasks: Vec<Task>,
}

impl State {
    fn new() -> Self {
        State {
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

    fn replace_current(mut self, new_task: Task) -> Self {
        self.tasks[self.ptr] = new_task;
        self
    }

    fn replace_task(mut self, task_id: usize, new_task: Task) -> Self {
        self.tasks[task_id] = new_task;
        self
    }

    fn get_children(&self) -> Vec<(usize, &Task)> {
        let parent = self.get_current();
        let children = &parent.children;
        children
            .iter()
            .map(|&index| (index, self.tasks.get(index).unwrap()))
            .collect()
    }

    fn add(mut self, args: &ArgMatches) -> Self {
        let task = Task::new()
            .with_name(String::from(args.value_of("name").unwrap()))
            .set_parent(self.ptr);

        let index_of_child = self.tasks.len();
        let new_parent = self.get_current_owned().add_child(index_of_child);
        self.tasks.push(task);
        self.replace_current(new_parent)
    }

    fn ascend(mut self) -> Self {
        if self.ptr == 0 {
            return self;
        }

        let parent = self.get_current().parent.unwrap(); // Panics if a task points to an invalid parent position
        self.ptr = parent;
        self
    }

    fn complete(self, args: &ArgMatches) -> Self {
        let task_id = match args.value_of("id").unwrap().parse::<usize>() {
            Ok(id) => id,
            Err(e) => {
                println!("{}", e);
                return self;
            }
        };

        let completed_task = self.get_task_owned(task_id).unwrap().complete(); // Panics if the supplied task id does not exist
        self.replace_task(task_id, completed_task)
    }

    fn delete(mut self, args: &ArgMatches) -> Self {
        let remove_id = match args.value_of("id").unwrap().parse::<usize>() {
            Ok(id) => id,
            Err(e) => {
                println!("{}", e);
                return self;
            }
        };

        if remove_id == 0 {
            return self;
        }

        let mut parent_id = self.get_task(remove_id).unwrap().parent.unwrap(); // Panics if supplied task does not exist and if it's parent does not exist
        let mut new_parent = self
            .get_task_owned(parent_id)
            .unwrap() // Panics if the task given does not exist
            .remove_child(remove_id);
        self = self.replace_task(parent_id, new_parent);
        self.tasks.swap_remove(remove_id);

        // The task we just swap_removed has the index tasks.len() - 1 before
        // we removed it, hence it is just tasks.len() now. We now need to
        // update the swapped task's parent to point to the correct index.
        let id_to_replace = self.tasks.len();
        parent_id = match self.get_task(remove_id) {
            Some(id) => id.parent.unwrap(), // Panics if the task's parent does not exist
            // If the location we just swap_removed does not exist, the task to
            // delete was the last in the list and we can stop here
            None => return self,
        };
        new_parent = self
            .get_task_owned(parent_id)
            .unwrap() // Panics if the current task does not exist
            .replace_child(id_to_replace, remove_id);
        self.replace_task(parent_id, new_parent)
    }

    fn descend(mut self, args: &ArgMatches) -> Self {
        let descend_id = match args.value_of("id").unwrap().parse::<usize>() {
            Ok(id) => id,
            Err(e) => {
                println!("{}", e);
                return self;
            }
        };

        if self.get_current().is_child(descend_id) {
            self.ptr = descend_id;
        }

        self
    }

    fn list(&self) {
        if !self.at_root() {
            println!("Task - {}", self.get_current());
        }

        for (id, task) in self
            .get_children()
            .iter()
            .filter(|(_, task)| !task.is_complete())
        {
            println!("{}. {}", id, task);
        }
    }
}

const FILE_NAME: &'static str = ".toru.yaml";

fn main() {
    let clap_config = load_yaml!("../cli.yaml");
    let matches = App::from_yaml(clap_config).get_matches();

    let mut state = match File::open(FILE_NAME) {
        Ok(file) => serde_yaml::from_reader(file).expect("Error deserialising"),
        Err(_) => State::new(),
    };

    state = match matches.subcommand() {
        ("add", Some(sub_matches)) => state.add(sub_matches),
        ("up", Some(_)) => state.ascend(),
        ("complete", Some(sub_matches)) => state.complete(sub_matches),
        ("delete", Some(sub_matches)) => state.delete(sub_matches),
        ("down", Some(sub_matches)) => state.descend(sub_matches),
        ("list", Some(_)) => {
            state.list();
            state
        }
        _ => state,
    };

    serde_yaml::to_writer(File::create(FILE_NAME).unwrap(), &state)
        .expect("Error serialising")
}
