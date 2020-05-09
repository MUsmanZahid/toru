use crate::task::Task;
use crate::tree::{self, Tree};
use crossterm::{
    cursor::{self, MoveTo},
    event::{poll, read, Event, KeyCode, KeyEvent, KeyModifiers},
    style::Print,
    terminal::{
        self, disable_raw_mode, enable_raw_mode, Clear, ClearType,
        EnterAlternateScreen, LeaveAlternateScreen,
    },
    ExecutableCommand, QueueableCommand,
};
use std::{env, error::Error, fs::File, io, path::Path, time::Duration};
use tui::{
    backend::CrosstermBackend,
    layout::{Constraint, Layout},
    style::{Modifier, Style},
    widgets::{Block, Borders, List, ListState, Text},
    Terminal,
};

#[derive(PartialEq)]
enum InputType {
    Direct,
    Text,
}

struct Input {
    prompt: String,
    cursor_offset: usize,
    input_type: InputType,
    buffer: String,
}

impl Default for Input {
    fn default() -> Self {
        Self {
            prompt: String::from("Input:"),
            cursor_offset: 0,
            input_type: InputType::Direct,
            buffer: String::with_capacity(40),
        }
    }
}

pub fn run() -> Result<(), Box<dyn Error>> {
    let file_name = Path::new(".toru.yaml");
    let key = if cfg!(windows) { "HOMEPATH" } else { "HOME" };
    let path = match env::var(key) {
        Ok(home) => Path::new(&home).join(file_name),
        Err(_) => file_name.to_path_buf(),
    };

    let file = File::open(&path);
    let mut tree = if file.is_ok() {
        let file = file.unwrap();
        serde_yaml::from_reader::<_, Tree>(file)?
    } else {
        let tree = Tree::new();
        let task = Task::new()
            .set_name("Task 1".to_string())
            .set_parent(tree.ptr());
        tree::add(tree, task)
    };

    let mut input_state = Input::default();

    let mut out = io::stdout();
    enable_raw_mode()?;
    out.queue(cursor::Hide)?.queue(EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(out);
    let mut terminal = Terminal::new(backend)?;

    let mut list = rebuild_list(&tree);

    loop {
        terminal.draw(|mut f| {
            let frame_size = f.size();
            let chunks = Layout::default()
                .constraints([Constraint::Min(1), Constraint::Min(1)].as_ref())
                .split(frame_size);
            let items = list.items.iter().map(|i| Text::raw(i));
            let title = format!("Toru: {}", tree.current().name());
            let items = List::new(items)
                .block(
                    Block::default()
                        .title(title.as_str())
                        .title_style(Style::default().modifier(Modifier::BOLD))
                        .borders(Borders::ALL),
                )
                .style(Style::default())
                .highlight_style(Style::default().modifier(Modifier::REVERSED))
                .highlight_symbol(">");
            f.render_stateful_widget(items, chunks[0], &mut list.state);
        })?;

        if input_state.input_type == InputType::Text {
            let cur_offset = &mut input_state.cursor_offset;
            let buffer = &mut input_state.buffer;

            let in_prompt =
                format!("{}{}", input_state.prompt, buffer.as_str());
            let inlen = in_prompt.len();

            let (_, rows) = terminal::size()?;

            terminal
                .backend_mut()
                .queue(MoveTo(1, rows))?
                .queue(Print(in_prompt))?
                .queue(MoveTo((1 + inlen - *cur_offset) as u16, rows))?
                .execute(cursor::Show)?;

            if let Ok(true) = poll(Duration::from_millis(150)) {
                tree = handle_input(&mut input_state, tree, &mut list)?;
            }

            terminal
                .backend_mut()
                .queue(Clear(ClearType::CurrentLine))?
                .queue(cursor::Hide)?;
            continue;
        }

        if let Ok(true) = poll(Duration::from_millis(150)) {
            if let Ok(Event::Key(KeyEvent { code, .. })) = read() {
                match code {
                    KeyCode::Char('q') => break,
                    KeyCode::Char('c') => {
                        let current = list.state.selected().unwrap();
                        let idx = tree.nth_child(current)?;
                        tree = tree::complete(tree, idx);
                        list = rebuild_list(&tree);
                    }
                    KeyCode::Char('a') => {
                        input_state.input_type = InputType::Text;
                    }
                    KeyCode::Char('d') => {
                        let current = list.state.selected().unwrap();
                        let current = tree.nth_child(current)?;
                        tree = tree::delete(tree, current);
                        list = rebuild_list(&tree);
                    }
                    KeyCode::Left | KeyCode::Char('h') => {
                        tree = tree::ascend(tree);
                        list = rebuild_list(&tree);
                    }
                    KeyCode::Up | KeyCode::Char('k') => list.previous(),
                    KeyCode::Down | KeyCode::Char('j') => list.next(),
                    KeyCode::Right | KeyCode::Char('l') => {
                        let current = list.state.selected().unwrap();
                        let current = tree.nth_child(current)?;
                        tree = tree::descend(tree, current);
                        list = rebuild_list(&tree);
                    }
                    _ => {}
                }
            }
        }
    }

    let file = File::create(path)?;
    serde_yaml::to_writer(file, &tree)?;

    terminal
        .backend_mut()
        .queue(cursor::Show)?
        .execute(LeaveAlternateScreen)?;
    disable_raw_mode()?;

    Ok(())
}

fn rebuild_list(tree: &Tree) -> StatefulList {
    StatefulList::new(
        tree.pending_children()
            .map(|c| {
                if c.has_children() {
                    format!("+ {}", c.name().clone())
                } else {
                    format!("  {}", c.name().clone())
                }
            })
            .collect(),
    )
}

fn handle_input(
    input: &mut Input,
    mut tree: Tree,
    list: &mut StatefulList,
) -> Result<Tree, Box<dyn Error>> {
    let (cols, _) = terminal::size()?;
    let len = input.buffer.len();
    let in_len = len + input.prompt.len();
    if let Event::Key(KeyEvent { code, modifiers }) = read()? {
        match code {
            KeyCode::Right => {
                if input.cursor_offset > 0 {
                    input.cursor_offset -= 1;
                }
            }
            KeyCode::Left => {
                if input.cursor_offset < len {
                    input.cursor_offset += 1;
                }
            }
            KeyCode::Esc => {
                input.buffer.clear();
                input.input_type = InputType::Direct;
            }
            KeyCode::Delete => {
                let idx = len - input.cursor_offset;
                if idx < len {
                    input.buffer.remove(idx);
                    input.cursor_offset -= 1;
                }
            }
            KeyCode::Backspace => {
                let idx = len - input.cursor_offset;
                if 0 < idx && idx <= len {
                    input.buffer.remove(idx - 1);
                }
            }
            KeyCode::Enter => {
                input.input_type = InputType::Direct;
                input.cursor_offset = 0;
                let task = Task::new()
                    .set_name(input.buffer.clone())
                    .set_parent(tree.ptr());
                tree = tree::add(tree, task);
                input.buffer.clear();
                *list = rebuild_list(&tree);
            }
            KeyCode::Char(c) => {
                if modifiers == KeyModifiers::CONTROL && c == 'c' {
                    input.input_type = InputType::Direct;
                    input.buffer.clear();
                }
                let cols = (cols - 1) as usize;
                if cols > in_len {
                    input.buffer.push(c);
                }
            }
            _ => {}
        }
    }

    Ok(tree)
}

struct StatefulList {
    state: ListState,
    items: Vec<String>,
}

impl StatefulList {
    fn new(items: Vec<String>) -> StatefulList {
        let mut state = ListState::default();
        state.select(Some(0));
        StatefulList {
            state,
            items,
        }
    }

    fn next(&mut self) {
        let i = match self.state.selected() {
            Some(i) => {
                if i >= self.items.len() - 1 {
                    0
                } else {
                    i + 1
                }
            }
            None => 0,
        };

        self.state.select(Some(i))
    }

    pub fn previous(&mut self) {
        let i = match self.state.selected() {
            Some(i) => {
                if i == 0 {
                    self.items.len() - 1
                } else {
                    i - 1
                }
            }
            None => 0,
        };

        self.state.select(Some(i));
    }
}
