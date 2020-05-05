//! This module defines the [`Tree`] structure which forms the basis of all
//! operations on the task list.
//!
//! [`Tree`]: ./struct.Tree.html

use crate::task::Task;
use crate::ToruError;
use serde::{Deserialize, Serialize};

pub struct Children<'a> {
    current: usize,
    indexes: &'a Vec<usize>,
    tasks: &'a Vec<Task>,
}

impl<'a> Iterator for Children<'a> {
    type Item = &'a Task;

    fn next(&mut self) -> Option<Self::Item> {
        if self.current >= self.indexes.len() {
            None
        } else {
            self.current += 1;
            let idx = self.indexes.get(self.current - 1).unwrap();
            Some(self.tasks.get(*idx).unwrap())
        }
    }
}

#[derive(Serialize, Deserialize)]
pub struct Tree {
    ptr: usize,
    tasks: Vec<Task>,
}

impl Tree {
    pub fn new() -> Self {
        Self {
            ptr: 0,
            tasks: vec![Task::new()],
        }
    }

    pub fn ptr(&self) -> usize {
        self.ptr
    }

    pub fn set_ptr(&mut self, idx: usize) {
        self.ptr = idx
    }

    pub fn tasks(&self) -> &Vec<Task> {
        &self.tasks
    }

    pub fn tasks_mut(&mut self) -> &mut Vec<Task> {
        &mut self.tasks
    }

    pub fn at_root(&self) -> bool {
        self.ptr == 0
    }

    pub fn current(&self) -> &Task {
        &self.tasks[self.ptr]
    }

    pub fn current_owned(&self) -> Task {
        self.tasks[self.ptr].clone()
    }

    pub fn task(&self, id: usize) -> Option<&Task> {
        self.tasks.get(id)
    }

    pub fn task_owned(&self, id: usize) -> Option<Task> {
        self.tasks.get(id).and_then(|task| Some(task.clone()))
    }

    pub fn has_pending(&self, task: &Task) -> bool {
        self.children_of(task).any(|child| !child.is_complete())
    }

    pub fn replace_current(mut self, new_task: Task) -> Self {
        self.tasks[self.ptr] = new_task;
        self
    }

    pub fn replace_task(mut self, task_id: usize, new_task: Task) -> Self {
        self.tasks[task_id] = new_task;
        self
    }

    pub fn children(&self) -> Children<'_> {
        let parent = self.current();
        let children = parent.children();
        Children {
            current: 0,
            indexes: children,
            tasks: &self.tasks(),
        }
    }

    pub fn children_of<'a>(&'a self, task: &'a Task) -> Children<'_> {
        let children = task.children();
        Children {
            current: 0,
            indexes: children,
            tasks: &self.tasks(),
        }
    }

    pub fn pending_children(&self) -> impl Iterator<Item = &'_ Task> {
        self.children().filter(|child| !child.is_complete())
    }

    pub fn nth_child(&self, idx: usize) -> Result<usize, ToruError> {
        self.current()
            .children()
            .iter()
            .filter(|&&task_idx| {
                let task = self.task(task_idx).unwrap();
                !task.is_complete()
            })
            .nth(idx)
            .and_then(|&nth| Some(nth))
            .ok_or(ToruError::InvalidIndex(idx))
    }
}

pub fn add(mut tree: Tree, task: Task) -> Tree {
    let index_of_child = tree.tasks().len();
    let new_parent = tree.current_owned().add_child(index_of_child);
    tree.tasks_mut().push(task);
    tree.replace_current(new_parent)
}

pub fn ascend(mut tree: Tree) -> Tree {
    if tree.at_root() {
        return tree;
    }

    match tree.current().parent() {
        Some(parent) => tree.set_ptr(parent),
        None => unreachable!(),
    }

    tree
}

pub fn complete(tree: Tree, idx: usize) -> Tree {
    let complete_task = tree.task_owned(idx).unwrap().complete();
    tree.replace_task(idx, complete_task)
}

pub fn delete(mut tree: Tree, idx: usize) -> Tree {
    if idx == 0 {
        return tree;
    }

    let mut parent_idx = tree.task(idx).unwrap().parent().unwrap();
    let mut new_parent = tree.task_owned(parent_idx).unwrap().remove_child(idx);

    tree = tree.replace_task(parent_idx, new_parent);
    tree.tasks_mut().swap_remove(idx);

    // Need to update the parent of the task we just swap removed to point to
    // the correct index (the index we just deleted)
    let idx_to_replace = tree.tasks().len();
    parent_idx = match tree.task(idx) {
        Some(id) => id.parent().unwrap(),
        // If there is nothing at the index we just deleted then we were
        // deleting the last node in the tree
        None => return tree,
    };

    new_parent = tree
        .task_owned(parent_idx)
        .unwrap()
        .replace_child(idx_to_replace, idx);
    tree.replace_task(parent_idx, new_parent)
}

pub fn descend(mut tree: Tree, idx: usize) -> Tree {
    if tree.current().is_child(idx) {
        tree.set_ptr(idx);
    }

    tree
}
