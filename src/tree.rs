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
        if self.current < self.indexes.len() {
            let idx = self.indexes.get(self.current)?;
            self.current += 1;
            return self.tasks.get(*idx);
        }

        None
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
        self.tasks.get(id).cloned()
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

    pub fn children(&self) -> Children {
        let parent = self.current();
        let children = parent.children();
        Children {
            current: 0,
            indexes: children,
            tasks: self.tasks(),
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
        self.children().filter(|&child| !child.is_complete())
    }

    pub fn nth_child(&self, idx: usize) -> Result<usize, ToruError> {
        self.current()
            .children()
            .iter()
            .filter(|&&task_idx| {
                let task = self.task(task_idx).unwrap_or_else(|| {
                    panic!("Invalid access of task {}", task_idx)
                });
                !task.is_complete()
            })
            .nth(idx)
            .copied()
            .ok_or(ToruError::InvalidIndex(idx))
    }
}

pub fn add(mut tree: Tree, task: Task) -> Tree {
    let task = task.set_parent(tree.ptr());
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

pub fn complete(mut tree: Tree, idx: usize) -> Tree {
    if idx == 0 {
        return tree;
    }

    let mut ptr = 0;
    let mut stack = Vec::new();
    stack.push(idx);

    while ptr < stack.len() {
        let idx = stack[ptr];
        let completed_task =
            match tree.task_owned(idx).map(|task| task.complete()) {
                Some(t) => t,
                None => panic!("Invalid index access at {}", idx),
            };

        stack.extend(completed_task.children().iter());
        tree = tree.replace_task(idx, completed_task);
        ptr += 1;
    }

    tree
}

pub fn delete(mut tree: Tree, idx: usize) -> Tree {
    if idx == 0 {
        return tree;
    }

    let mut stack = Vec::new();
    stack.push(idx);

    while !stack.is_empty() {
        let child_index = match stack.last().cloned() {
            Some(i) => i,
            None => unreachable!(),
        };
        let child_task = match tree.task(child_index) {
            Some(t) => t,
            None => {
                stack.pop();
                continue;
            }
        };

        let children = child_task.children();

        if !children.is_empty() {
            stack.extend(children.iter());
            continue;
        }

        let mut parent_index = match tree.task(child_index) {
            Some(t) => match t.parent() {
                Some(p) => p,
                None => unreachable!(),
            },
            None => unreachable!(),
        };
        let mut new_parent = match tree.task_owned(parent_index) {
            Some(t) => t.remove_child(child_index),
            None => unreachable!(),
        };

        tree = tree.replace_task(parent_index, new_parent);
        tree.tasks_mut().swap_remove(child_index);

        // Need to update parent of the task we just swapped
        let index_to_replace = tree.tasks().len();
        parent_index = match tree.task(child_index) {
            Some(t) => t.parent().unwrap(),
            None => {
                // If there is no task at the one we just deleted we deleted the last node and don't
                // need to do anything else
                stack.pop();
                continue;
            }
        };
        new_parent = match tree.task_owned(parent_index) {
            Some(t) => t.replace_child(index_to_replace, child_index),
            None => unreachable!(),
        };
        tree = tree.replace_task(parent_index, new_parent);

        stack.pop();
    }

    tree
}

pub fn descend(mut tree: Tree, idx: usize) -> Tree {
    if tree.current().has_child_with_index(idx) {
        tree.set_ptr(idx);
    }

    tree
}

#[cfg(test)]
mod test {
    use super::*;

    fn spawn_tree() -> Tree {
        let mut tree = Tree::new();
        tree = add(tree, Task::new().set_parent(0));
        tree = add(tree, Task::new().set_parent(0));
        tree.set_ptr(1);
        tree = add(tree, Task::new().set_parent(1));
        tree = add(tree, Task::new().set_parent(1));
        tree.set_ptr(2);
        tree = add(tree, Task::new().set_parent(2));
        tree = add(tree, Task::new().set_parent(2));
        tree.set_ptr(3);
        tree = add(tree, Task::new().set_parent(3));
        tree = add(tree, Task::new().set_parent(3));
        tree.set_ptr(4);
        tree = add(tree, Task::new().set_parent(4));

        tree
    }

    #[test]
    fn multi_level_delete() {
        let mut tree = spawn_tree();
        tree = delete(tree, 1);

        assert_eq!(tree.tasks().len(), 4);
    }

    #[test]
    fn single_level_delete() {
        let mut tree = spawn_tree();
        tree = delete(tree, 9);

        assert_eq!(tree.tasks().len(), 9);
    }

    #[test]
    fn multi_level_complete() {
        let mut tree = spawn_tree();
        tree.set_ptr(1);

        tree = complete(tree, 1);
        assert!(tree.pending_children().next().is_none());
    }
}
