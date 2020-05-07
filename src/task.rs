//! This module defines the [`Task`] structure which is the basis for all
//! operations in toru.
//!
//! [`Task`]: ./struct.Task.html

use serde::{Deserialize, Serialize};
use std::fmt;
use time::PrimitiveDateTime;

#[derive(Serialize, Deserialize, Clone, Debug)]
/// Enum that represents the current state of a task. Currently this is
/// primarily used to differentiate between pending and completed tasks,
/// however, it may see some other uses in the future.
enum Status {
    Pending,
    Complete,
}

/// This is the core structure which holds all information about a task.
///
/// Toru uses unsigned integers instead of pointers as references to other
/// tasks. All indexing must be valid and any indexing panic is likely due to a
/// logic error. An individual task itself does very little and needs to work
/// with [`Tree`] in order to be useful.
///
/// [`Tree`]: ../tree/struct.Tree.html
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Task {
    /// The parent (if any) of the task. The only task that should not have a
    /// parent is the root task.
    #[doc(hidden)]
    parent: Option<usize>,
    /// The name of the task
    #[doc(hidden)]
    name: String,
    /// The due date (if any) of the task.
    #[doc(hidden)]
    due: Option<PrimitiveDateTime>,
    /// The status of the task. See the [`Status`] enum.
    /// [`Status`]: ./enum.Status.html
    #[doc(hidden)]
    status: Status,
    /// A vector of unsigned integers that holds the indices to the task's
    /// children.
    #[doc(hidden)]
    children: Vec<usize>,
}

impl Task {
    /// Creates a new task with 'default' instances of fields.
    /// # Examples
    ///
    /// ```
    /// let task = Task::new();
    /// assert_eq!(task.parent(), None);
    /// assert_eq!(task.name(), String::from("Root"));
    /// assert_eq!(task.due(), None);
    /// assert_eq!(task.status(), Status::Pending);
    /// assert_eq!(task.children(), vec![]);
    /// ```
    ///
    /// [`Pending`]: ./enum.Status.html
    pub fn new() -> Self {
        Task {
            parent: None,
            name: String::from("Root"),
            due: None,
            status: Status::Pending,
            children: Vec::new(),
        }
    }

    /// Returns an immutable reference to the name of a Task.
    ///
    /// # Examples
    ///
    /// ```
    /// let task = Task::new();
    /// assert_eq!(task.name(), &String::from("Default"));
    /// ```
    ///
    pub fn name(&self) -> &String {
        &self.name
    }

    /// Returns the due date (if any) of the task.
    ///
    /// # Examples
    ///
    /// ```
    /// use time::PrimitiveDateTime; // A dependency of toru
    ///
    /// let mut task = Task::new();
    /// assert_eq!(task.due(), None);
    ///
    /// let date = PrimitiveDateTime::new(date!(2019-01-01), time!(0:00));
    ///
    /// task.set_due(date);
    /// assert_eq!(task.due(), date!(2019-01-01).midnight());
    /// ```
    ///
    pub fn due(&self) -> &Option<PrimitiveDateTime> {
        &self.due
    }

    /// Returns the parent of a task.
    ///
    /// # Examples
    ///
    /// ```
    /// let mut task = Task::new();
    /// assert_eq!(task.parent(), None);
    ///
    /// task = task.set_parent(1);
    /// assert_eq!(task.parent(), Some(1));
    /// ```
    ///
    pub fn parent(&self) -> Option<usize> {
        self.parent
    }

    /// Returns an immutable reference to a task's children.
    ///
    /// # Examples
    ///
    /// ```
    /// let mut task = Task::new();
    /// assert_eq!(task.children(), vec![])
    ///
    /// task = task.add_child(1);
    /// assert_eq!(task.children(), vec![1]);
    /// ```
    ///
    pub fn children(&self) -> &Vec<usize> {
        &self.children
    }

    /// Sets the parent of the task.
    ///
    /// # Examples
    ///
    /// ```
    /// let mut task = Task::new();
    /// assert_eq!(task.parent(), None);
    ///
    /// task = task.set_parent(3);
    /// assert_eq!(task.parent(), Some(3));
    /// ```
    ///
    pub fn set_parent(mut self, parent_index: usize) -> Self {
        self.parent = Some(parent_index);
        self
    }

    /// Sets the name of the task.
    ///
    /// # Examples
    ///
    /// ```
    /// let mut task = Task::new();
    /// assert_eq!(task.name(), "Default".to_string());
    ///
    /// let name = "Hello!".to_string();
    /// task = task.set_name(name.clone());
    /// assert_eq!(task.name(), name);
    /// ```
    ///
    pub fn set_name(mut self, name: String) -> Self {
        self.name = name;
        self
    }

    /// Sets the due date of the task.
    ///
    /// # Examples
    ///
    /// ```
    /// use time::PrimitiveDateTime; // A dependency of toru
    ///
    /// let mut task = Task::new();
    /// let date = PrimitiveDateTime::new(date!(2019-01-01), time!(0:00));
    ///
    /// assert_eq!(task.due(), None);
    ///
    /// task = task.set_due(date);
    /// assert_eq!(task.due(), date!(2019-01-01).midnight());
    ///
    /// ```
    ///
    pub fn set_due(mut self, date: PrimitiveDateTime) -> Self {
        self.due = Some(date);
        self
    }

    /// Complete a task.
    ///
    /// # Examples
    ///
    /// ```
    /// let mut task = Task::new();
    /// assert!(!task.is_complete());
    ///
    /// task = task.complete();
    /// assert!(task.is_complete());
    /// ```
    ///
    pub fn complete(mut self) -> Task {
        self.status = Status::Complete;
        self
    }

    /// Checks whether a task is complete or not.
    ///
    /// # Examples
    ///
    /// ```
    /// let mut task = Task::new();
    /// assert!(!task.is_complete());
    ///
    /// task = task.complete();
    /// assert!(task.is_complete());
    /// ```
    ///
    pub fn is_complete(&self) -> bool {
        if let Status::Complete = self.status {
            true
        } else {
            false
        }
    }

    /// Add a child index to a task's children.
    ///
    /// # Examples
    ///
    /// ```
    /// let mut task = Task::new();
    /// assert_eq!(task.children(), vec![])
    ///
    /// task = task.add_child(1);
    /// assert_eq!(task.children(), vec![1]);
    /// ```
    ///
    pub fn add_child(mut self, child_index: usize) -> Self {
        self.children.push(child_index);
        self
    }

    /// Removes a child index from a task's children.
    ///
    /// # Examples
    ///
    /// ```
    /// let mut task = Task::new();
    /// assert_eq!(task.children(), vec![])
    ///
    /// task = task.add_child(1);
    /// assert_eq!(task.children(), vec![1]);
    ///
    /// task = task.remove_child(1);
    /// assert_eq!(task.children(), vec![]);
    /// ```
    ///
    pub fn remove_child(mut self, child_index: usize) -> Self {
        self.children.retain(|&index| index != child_index);
        self
    }

    /// Replace a child index with another index.
    ///
    /// # Examples
    ///
    /// ```
    /// let mut task = Task::new().add_child(1).add_child(2);
    /// task = task.replace_child(1, 3);
    ///
    /// assert_eq!(task.children(), vec![3, 2]);
    /// ```
    ///
    pub fn replace_child(mut self, old_child: usize, new_child: usize) -> Self {
        let new_children: Vec<usize> = self
            .children
            .iter()
            .map(|&index| if index == old_child { new_child } else { index })
            .collect();

        self.children = new_children;
        self
    }

    /// Checks whether a task has children or not.
    ///
    /// # Examples
    ///
    /// ```
    /// let mut task = Task::new();
    /// assert!(!task.has_children());
    ///
    /// task = task.add_child(1);
    /// assert!(task.has_children())
    /// ```
    ///
    pub fn has_children(&self) -> bool {
        !self.children.is_empty()
    }

    /// Checks whether an index is part of the task's children.
    ///
    /// # Examples
    ///
    /// ```
    /// let task = Task::new().add_child(1);
    /// assert!(task.is_child(1));
    /// assert!(!task.is_child(2));
    /// ```
    ///
    pub fn is_child(&self, id: usize) -> bool {
        self.children.contains(&id)
    }
}

impl fmt::Display for Task {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.due() {
            Some(due) => {
                write!(f, "{} | {}", self.name, due.format("%I:%M %p %F"))
            }
            None => write!(f, "{}", self.name),
        }
    }
}
