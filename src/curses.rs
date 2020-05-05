use crate::task::Task;
use crate::tree::{self, Tree};
use crossterm::{
    cursor,
    event::{poll, read, Event, KeyCode},
    style::Print,
    terminal::{
        disable_raw_mode, enable_raw_mode, Clear, ClearType,
        EnterAlternateScreen, LeaveAlternateScreen,
    },
    QueueableCommand,
};
use std::{
    env,
    error::Error,
    fs::File,
    io::{self, Write},
    path::Path,
    time::Duration,
};
use tui::{
    backend::{Backend, CrosstermBackend},
    layout::{Constraint, Layout},
    style::{Color, Modifier, Style},
    widgets::{Block, List, ListState, Text},
    Terminal,
};

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
        Tree::new()
    };

    let out = io::stdout();
    let mut stdout = out.lock();
    enable_raw_mode()?;
    stdout.queue(cursor::Hide)?.queue(EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let rebuild_list = |tree: &Tree| {
        StatefulList::new(
            tree.pending_children().map(|c| c.name().clone()).collect(),
        )
    };

    let mut list = rebuild_list(&tree);

    loop {
        terminal.draw(|mut f| {
            let frame_size = f.size();
            let chunks = Layout::default()
                .constraints([Constraint::Min(0), Constraint::Max(2)].as_ref())
                .split(frame_size);
            let items = list.items.iter().map(|i| Text::raw(i));
            let title = format!("Toru: {}", tree.current().name());
            let items = List::new(items)
                .block(Block::default().title(title.as_str()))
                .style(Style::default())
                .highlight_style(
                    Style::default()
                        .fg(Color::LightGreen)
                        .modifier(Modifier::BOLD),
                )
                .highlight_symbol(">");
            f.render_stateful_widget(items, chunks[0], &mut list.state);
        })?;

        if let Ok(true) = poll(Duration::from_millis(150)) {
            if let Ok(Event::Key(key)) = read() {
                match key.code {
                    KeyCode::Char('q') => break,
                    KeyCode::Char('c') => {
                        let current = list.state.selected().unwrap();
                        let idx = tree.nth_child(current)?;
                        tree = tree::complete(tree, idx);
                        list = rebuild_list(&tree);
                    }
                    KeyCode::Char('a') => {
                        let name = get_input(terminal.backend_mut(), "Name: ")?;
                        if name == "\0" {
                            continue;
                        }
                        let task =
                            Task::new().set_name(name).set_parent(tree.ptr());
                        tree = tree::add(tree, task);
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
                    KeyCode::Left | KeyCode::Char('h') => {
                        tree = tree::ascend(tree);
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
        .queue(LeaveAlternateScreen)?;
    disable_raw_mode()?;

    Ok(())
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
            state: state,
            items: items,
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

fn get_input<B>(stdout: &mut B, prompt: &str) -> Result<String, Box<dyn Error>>
where
    B: Backend + Write,
{
    let (_, rows) = crossterm::terminal::size()?;
    let mut buffer = String::with_capacity(40);

    stdout
        .queue(cursor::MoveTo(1, rows))?
        .queue(Print(format!("{}", prompt)))?
        .queue(cursor::Show)?;
    Write::flush(stdout)?;

    let (zero_col, _) = cursor::position()?;

    loop {
        if let Event::Key(event) = read()? {
            match event.code {
                KeyCode::Backspace => {
                    let (cur_col, _) = cursor::position()?;
                    if cur_col == zero_col {
                        buffer.push('\0');
                        break;
                    }
                    buffer.pop();
                    stdout
                        .queue(cursor::MoveLeft(1))?
                        .queue(Print(" "))?
                        .queue(cursor::MoveLeft(1))?;
                    Write::flush(stdout)?;
                }
                KeyCode::Enter => break,
                KeyCode::Esc => {
                    buffer.clear();
                    buffer.push('\0');
                    break;
                }
                KeyCode::Char(c) => {
                    buffer.push(c);
                    write!(stdout, "{}", c)?;
                    Write::flush(stdout)?;
                }
                _ => {}
            }
        }
    }

    stdout
        .queue(cursor::Hide)?
        .queue(Clear(ClearType::CurrentLine))?;
    Write::flush(stdout)?;
    Ok(buffer)
}
