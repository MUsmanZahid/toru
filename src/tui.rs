use crate::task::Task;
use crate::tree::{self, Tree};
use crate::Result;

use std::{
    fmt,
    io::{self, Write},
    sync::mpsc::{self, Receiver},
    thread,
    time::Duration,
};
use termion::{
    clear, cursor,
    event::{self, Key},
    input::TermRead,
    raw::IntoRawMode,
    screen::AlternateScreen,
    terminal_size,
};

const RESIZE_POLL_TIMEOUT: Duration = Duration::from_millis(150);

#[derive(Debug, PartialEq)]
enum Event {
    Resize(u16, u16),
    Key(event::Key),
}

enum State {
    Normal,
    Input,
    Mutate(Action),
    Exit,
}

enum Action {
    AddTask,
    DeleteTask,
}

struct List {
    index: usize,
    title: String,
    items: Vec<String>,
}

impl List {
    fn new(title: String, items: Vec<String>) -> Self {
        Self {
            index: 0,
            title,
            items,
        }
    }

    fn rebuild(&mut self, tree: &Tree) {
        let title = tree.current().name().clone();
        let items: Vec<String> =
            tree.pending_children().map(|t| t.name().clone()).collect();
        let length = items.len();

        if length == 0 {
            self.index = length;
        } else if self.index >= length {
            self.index = length - 1;
        }

        self.title = title;
        self.items = items;
    }

    fn increment(&mut self) {
        let length = self.items.len();

        if 0 < length && self.index < length - 1 {
            self.index += 1;
        }
    }

    fn decrement(&mut self) {
        if self.index > 0 {
            self.index -= 1;
        }
    }
}

impl fmt::Display for List {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}\r\n", self.title)?;
        for (idx, item) in self.items.iter().enumerate() {
            let prompt = if self.index == idx { ">" } else { " " };
            write!(f, "{}. {} {}\r\n", idx + 1, prompt, item)?;
        }

        Ok(())
    }
}

struct App<W: Write> {
    output: W,
    cursor_offset: usize,
    list: List,
    buffer: String,
}

impl<W: Write> App<W> {
    fn new(output: W, title: String, items: Vec<String>) -> Self {
        Self {
            output,
            cursor_offset: 0,
            list: List::new(title, items),
            buffer: String::with_capacity(40),
        }
    }
}

pub fn run(mut tree: Tree) -> Result<Tree> {
    // Set up the channel
    let rx = spawn_event_threads();
    let output = AlternateScreen::from(io::stdout().into_raw_mode()?);

    let mut state = State::Normal;
    let title = tree.task(tree.ptr()).unwrap().name().clone();
    let items = tree.pending_children().map(|t| t.name().clone()).collect();
    let mut app = App::new(output, title, items);

    let output = &mut app.output;

    write!(output, "{}\r{}{}", clear::All, app.list, cursor::Hide)?;
    output.flush()?;

    tree = normal_state(&rx, &mut state, app, tree)?;

    Ok(tree)
}

fn spawn_event_threads() -> Receiver<Event> {
    let (tx, rx) = mpsc::channel::<Event>();
    let txc = tx.clone();

    // This thread only sends termion key events
    thread::spawn(move || {
        for event in io::stdin().events() {
            if let Ok(event::Event::Key(key)) = event {
                txc.send(Event::Key(key)).unwrap();
            }
        }
    });

    // This thread polls for terminal resize events
    thread::spawn(move || {
        let (mut previous_x, mut previous_y) = match terminal_size() {
            Ok(a) => a,
            _ => unreachable!(),
        };

        loop {
            let (current_x, current_y) = match terminal_size() {
                Ok(a) => a,
                _ => unreachable!(),
            };
            if current_x != previous_x || current_y != previous_y {
                previous_x = current_x;
                previous_y = current_y;

                tx.send(Event::Resize(current_x, current_y)).unwrap();
            }

            thread::sleep(RESIZE_POLL_TIMEOUT);
        }
    });

    rx
}

fn redraw<W: Write>(output: &mut W, list: &List) -> Result<()> {
    write!(
        output,
        "{}{}{}{}{}",
        cursor::Save,
        clear::All,
        cursor::Goto(1, 1),
        list,
        cursor::Restore
    )?;
    output.flush()?;

    Ok(())
}

fn normal_state<W: Write>(
    rx: &Receiver<Event>,
    state: &mut State,
    mut app: App<W>,
    mut tree: Tree,
) -> Result<Tree> {
    for received in rx {
        match received {
            Event::Resize(_x, _y) => {
                redraw(&mut app.output, &app.list)?;
            }
            Event::Key(key) => match key {
                Key::Left | Key::Char('h') => {
                    tree = tree::ascend(tree);
                    app.list.rebuild(&tree);
                }
                Key::Char('~') => {
                    tree.set_ptr(0);
                    app.list.rebuild(&tree);
                }
                Key::Right | Key::Char('l') => {
                    let index = app.list.index;
                    let selected_child = match tree.nth_child(index) {
                        Ok(child) => child,
                        Err(_) => continue,
                    };
                    tree = tree::descend(tree, selected_child);
                    app.list.rebuild(&tree);
                }
                Key::Up | Key::Char('k') => {
                    app.list.decrement();
                }
                Key::Down | Key::Char('j') => {
                    app.list.increment();
                }
                Key::Char('d') => {
                    *state = State::Mutate(Action::DeleteTask);
                    tree = mutate_state(state, &mut app, tree);
                    app.list.rebuild(&tree);
                }
                Key::Char('i') => {
                    let (_cols, rows) = terminal_size()?;
                    write!(
                        &mut app.output,
                        "{}{}",
                        cursor::Goto(1, rows),
                        cursor::Show
                    )?;
                    app.output.flush()?;
                    *state = State::Input;
                    tree = input_state("Name:", &rx, state, &mut app, tree)?;
                }
                Key::Char('q') => {
                    *state = State::Exit;
                    break;
                }
                _ => {}
            },
        }

        redraw(&mut app.output, &app.list)?;
    }

    write!(&mut app.output, "{}", cursor::Show)?;
    Ok(tree)
}

fn input_state<W: Write>(
    prompt: &str,
    rx: &Receiver<Event>,
    state: &mut State,
    app: &mut App<W>,
    mut tree: Tree,
) -> Result<Tree> {
    write!(app.output, "{}", prompt)?;
    app.output.flush()?;

    for received in rx {
        match received {
            Event::Resize(_new_x, new_y) => {
                redraw(&mut app.output, &app.list)?;
                write!(
                    app.output,
                    "{}{}{}{}",
                    clear::CurrentLine,
                    cursor::Goto(1, new_y),
                    prompt,
                    app.buffer,
                )?;
                if app.cursor_offset != 0 {
                    write!(
                        app.output,
                        "{}",
                        cursor::Left(app.cursor_offset as u16)
                    )?;
                }
            }
            Event::Key(key) => match key {
                Key::Esc => {
                    app.buffer.clear();
                    *state = State::Exit;
                    break;
                }
                Key::Delete => {
                    if app.cursor_offset > 0 {
                        let index = app.buffer.len() - app.cursor_offset;
                        app.buffer.remove(index);
                        app.cursor_offset -= 1;
                        write!(
                            app.output,
                            "{}{} {}",
                            cursor::Save,
                            &app.buffer[index..],
                            cursor::Restore
                        )?;
                    }
                }
                Key::Backspace | Key::Ctrl('h') => {
                    let length = app.buffer.len();
                    if app.cursor_offset < length {
                        let index = length - app.cursor_offset - 1;
                        app.buffer.remove(index);
                        write!(
                            app.output,
                            "{}{}{} {}",
                            cursor::Left(1),
                            cursor::Save,
                            &app.buffer[index..],
                            cursor::Restore
                        )?;
                    }
                }
                Key::Right | Key::Ctrl('f') => {
                    if app.cursor_offset > 0 {
                        app.cursor_offset -= 1;
                        write!(app.output, "{}", cursor::Right(1))?;
                    }
                }
                Key::Left | Key::Ctrl('b') => {
                    if app.cursor_offset < app.buffer.len() {
                        app.cursor_offset += 1;
                        write!(app.output, "{}", cursor::Left(1))?;
                    }
                }
                Key::Char('\n') | Key::Ctrl('j') => {
                    write!(
                        app.output,
                        "{}{}\r",
                        clear::CurrentLine,
                        cursor::Hide
                    )?;
                    app.output.flush()?;

                    if let State::Input = *state {
                        *state = State::Mutate(Action::AddTask);
                    }
                    tree = mutate_state(state, app, tree);
                    app.buffer.clear();

                    app.list.rebuild(&tree);
                    if let State::Normal = *state {
                        break;
                    }
                    redraw(&mut app.output, &app.list)?;
                }
                Key::Char(c) => {
                    app.buffer.push(c);
                    write!(app.output, "{}", c)?;
                }
                _ => {}
            },
        }
        app.output.flush()?;
    }

    write!(app.output, "{}\r{}", clear::CurrentLine, cursor::Hide)?;
    Ok(tree)
}

fn mutate_state<W: Write>(
    state: &mut State,
    app: &mut App<W>,
    mut tree: Tree,
) -> Tree {
    match state {
        State::Mutate(action) => match action {
            Action::AddTask => {
                let task = Task::new().set_name(app.buffer.clone());
                tree = tree::add(tree, task);
                *state = State::Normal;
            }
            Action::DeleteTask => {
                let current_index = app.list.index;
                let task_index = match tree.nth_child(current_index) {
                    Ok(a) => a,
                    _ => unreachable!(),
                };
                tree = tree::delete(tree, task_index);
            }
        },
        _ => unreachable!(),
    }

    tree
}
