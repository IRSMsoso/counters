use anyhow::Context;
use ratatui::crossterm::event;
use ratatui::crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind};
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, HighlightSpacing, List, ListItem, ListState, Paragraph};
use ratatui::Terminal;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::env::current_dir;
use std::fs::File;
use std::io;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use tui_input::backend::crossterm::EventHandler;
use tui_input::Input;

enum AddingModeSign {
    Positive,
    Negative,
}

enum NormalFocus {
    Increment,
    History,
}

enum InputMode {
    Normal(NormalFocus),
    NewCounter(Input),
    Adding(Input, AddingModeSign),
}

#[derive(Serialize, Deserialize)]
enum HistoryEntry {
    AddCounter(String),
    RemoveCounter(String),
    AddDelta(String, i64),
    SetValue(String, i64),
}

impl From<&HistoryEntry> for ListItem<'_> {
    fn from(entry: &HistoryEntry) -> Self {
        let line = Line::styled(
            match entry {
                HistoryEntry::AddCounter(name) => format!("Added {}", name),
                HistoryEntry::RemoveCounter(name) => format!("Removed {}", name),
                HistoryEntry::AddDelta(name, delta) => format!("{} {}", name, delta),
                HistoryEntry::SetValue(name, value) => format!("{} = {}", name, value),
            },
            Color::White,
        );

        ListItem::new(line)
    }
}

struct History(Vec<HistoryEntry>);

impl History {
    fn new() -> Self {
        Self(Vec::new())
    }

    fn add_entry(&mut self, entry: HistoryEntry) {
        self.0.push(entry);
    }

    fn calculate_counter_results(&self) -> BTreeMap<String, i64> {
        let mut counters = BTreeMap::new();
        for history_entry in &self.0 {
            match history_entry {
                HistoryEntry::AddCounter(name) => {
                    if !counters.contains_key(name) {
                        counters.insert(name.to_owned(), 0);
                    }
                }
                HistoryEntry::RemoveCounter(name) => {
                    counters.remove(name);
                }
                HistoryEntry::AddDelta(name, delta) => {
                    if let Some(value) = counters.get_mut(name) {
                        *value += *delta;
                    }
                }
                HistoryEntry::SetValue(name, new_value) => {
                    if let Some(value) = counters.get_mut(name) {
                        *value = *new_value;
                    }
                }
            }
        }
        counters
    }
}

struct Counter {
    name: String,
    value: i64,
}

enum SaveState {
    DoNotSave,
    Save(PathBuf),
}

pub(crate) struct App {
    history: History,
    history_list_state: ListState,
    counter_list_state: ListState, // counter_list is derived from history, thus no vec.
    input_mode: InputMode,
    should_exit: bool,
    save_state: SaveState,
}
impl App {
    pub(crate) fn make_temporary() -> Self {
        Self {
            history: History::new(),
            history_list_state: Default::default(),
            counter_list_state: Default::default(),
            input_mode: InputMode::Normal(NormalFocus::Increment),
            should_exit: false,
            save_state: SaveState::DoNotSave,
        }
    }

    pub(crate) fn make_saved(input_name: &str) -> anyhow::Result<Self> {
        let mut path = current_dir().context("Couldn't get working directory")?;
        path.push(input_name);
        path.set_extension("json");
        let file_exists = Path::exists(&path);

        Ok(if file_exists {
            let file =
                File::open(&path).context(format!("Failed to open file: {}", path.display()))?;
            let history: Vec<HistoryEntry> = serde_json::from_reader(file)
                .context(format!("Failed to parse file: {}", path.display()))?;

            Self {
                history: History(history),
                history_list_state: Default::default(),
                counter_list_state: Default::default(),
                input_mode: InputMode::Normal(NormalFocus::Increment),
                should_exit: false,
                save_state: SaveState::Save(path),
            }
        } else {
            Self {
                history: History::new(),
                history_list_state: Default::default(),
                counter_list_state: Default::default(),
                input_mode: InputMode::Normal(NormalFocus::Increment),
                should_exit: false,
                save_state: SaveState::Save(path),
            }
        })
    }

    fn save(&self) -> anyhow::Result<()> {
        let SaveState::Save(buf) = &self.save_state else {
            return Ok(());
        };

        let file = File::create(buf).context(format!("Failed to open file: {}", buf.display()))?;

        serde_json::to_writer_pretty(file, &self.history.0)
            .context(format!("Failed to open file: {}", buf.display()))?;

        Ok(())
    }

    pub(crate) fn run(&mut self, mut terminal: Terminal<impl Backend>) -> io::Result<String> {
        let mut end_message = String::new();

        while !self.should_exit {
            terminal.draw(|f| f.render_widget(&mut *self, f.size()))?;
            if let Event::Key(key) = event::read()? {
                match self.handle_key(key) {
                    Ok(_) => {}
                    Err(error) => {
                        end_message = error.to_string();
                    }
                };
            };
        }
        Ok(end_message)
    }

    fn handle_key(&mut self, key: KeyEvent) -> anyhow::Result<()> {
        if key.kind != KeyEventKind::Press {
            return Ok(());
        }
        match &mut self.input_mode {
            InputMode::Normal(normal_focus) => match (key.code, normal_focus) {
                (KeyCode::Up | KeyCode::Char('k'), normal_focus) => match normal_focus {
                    NormalFocus::Increment => self.counter_list_state.select_previous(),
                    NormalFocus::History => self.history_list_state.select_previous(),
                },
                (KeyCode::Down | KeyCode::Char('j'), normal_focus) => match normal_focus {
                    NormalFocus::Increment => self.counter_list_state.select_next(),
                    NormalFocus::History => self.history_list_state.select_next(),
                },
                (KeyCode::Right | KeyCode::Char('l'), NormalFocus::Increment) => {
                    match self.counter_list_state.selected() {
                        Some(index) => {
                            let counters = self.history.calculate_counter_results();
                            if let Some((name, value)) = counters.iter().nth(index) {
                                self.history
                                    .add_entry(HistoryEntry::AddDelta(name.to_owned(), 1));
                            }
                        }
                        None => {}
                    }
                    self.save()?;
                }
                (KeyCode::Left | KeyCode::Char(';'), NormalFocus::Increment) => {
                    match self.counter_list_state.selected() {
                        Some(index) => {
                            let counters = self.history.calculate_counter_results();
                            if let Some((name, value)) = counters.iter().nth(index) {
                                self.history
                                    .add_entry(HistoryEntry::AddDelta(name.to_owned(), -1));
                            }
                        }
                        None => {}
                    }
                    self.save()?;
                }
                (KeyCode::Char('q'), _) => self.should_exit = true,
                (KeyCode::Char('n'), _) => {
                    self.input_mode = InputMode::NewCounter(Input::default())
                }
                (KeyCode::Char('d'), NormalFocus::Increment) => {
                    match self.counter_list_state.selected() {
                        Some(index) => {
                            let counters = self.history.calculate_counter_results();
                            if let Some((name, value)) = counters.iter().nth(index) {
                                self.history
                                    .add_entry(HistoryEntry::RemoveCounter(name.to_owned()));
                            }
                        }
                        None => {}
                    }
                    self.save()?;
                }
                (KeyCode::Esc, NormalFocus::Increment) => self.counter_list_state.select(None),
                (KeyCode::Char('a'), _) => {
                    self.input_mode = InputMode::Adding(Input::default(), AddingModeSign::Positive)
                }
                (KeyCode::Char('s'), _) => {
                    self.input_mode = InputMode::Adding(Input::default(), AddingModeSign::Negative)
                }
                _ => {}
            },
            InputMode::NewCounter(input) => match key.code {
                KeyCode::Esc => self.input_mode = InputMode::Normal(NormalFocus::Increment),
                KeyCode::Enter => {
                    let counters = self.history.calculate_counter_results();
                    if !counters.contains_key(input.value()) {
                        self.history
                            .add_entry(HistoryEntry::AddCounter(input.value().to_owned()));
                        input.reset();
                        self.save()?;
                    }
                }
                _ => {
                    input.handle_event(&Event::Key(key));
                }
            },
            InputMode::Adding(input, sign) => match key.code {
                KeyCode::Up | KeyCode::Char('k') => self.counter_list_state.select_previous(),
                KeyCode::Down | KeyCode::Char('j') => self.counter_list_state.select_next(),
                KeyCode::Char(char) if char.is_numeric() => {
                    input.handle_event(&Event::Key(key));
                }
                KeyCode::Right | KeyCode::Left | KeyCode::Backspace => {
                    input.handle_event(&Event::Key(key));
                }
                KeyCode::Esc => self.input_mode = InputMode::Normal(NormalFocus::Increment),
                KeyCode::Enter => match self.counter_list_state.selected() {
                    Some(index) => {
                        let delta =
                            u64::from_str(input.value()).expect("String should only have numerics");

                        let counters = self.history.calculate_counter_results();
                        if let Some((name, value)) = counters.iter().nth(index) {
                            self.history.add_entry(HistoryEntry::AddDelta(
                                name.to_owned(),
                                match sign {
                                    AddingModeSign::Positive => delta as i64,
                                    AddingModeSign::Negative => (delta as i64) * -1,
                                },
                            ));

                            input.reset();
                            self.save()?;
                        }
                    }
                    None => {}
                },
                KeyCode::Char('a') => {
                    self.input_mode = InputMode::Adding(input.clone(), AddingModeSign::Positive)
                }
                KeyCode::Char('s') => {
                    self.input_mode = InputMode::Adding(input.clone(), AddingModeSign::Negative)
                }
                _ => {}
            },
        }
        Ok(())
    }

    fn render_footer(&self, area: Rect, buf: &mut Buffer) {
        // TODO: PROPER HELP
        // let description = match &self.input_mode {
        //     InputMode::Normal => {
        //         if self.counter_list.counters.is_empty() {
        //             "Use n to make a new counter, and q to exit."
        //         } else {
        //             "Use ↓↑/jk to move, d to delete, ←→/l; to increment the counter, n to make a new counter, a/s to add/subtract, and q to exit."
        //         }
        //     }
        //     InputMode::NewCounter(_) => {
        //         "Type a new counter name. Use enter to add and esc to return."
        //     }
        //     InputMode::Adding(_, sign) => match sign {
        //         AddingModeSign::Positive => {
        //             "Use ↓↑/jk to move, Type numbers, then enter to add and esc to return"
        //         }
        //         AddingModeSign::Negative => {
        //             "Use ↓↑/jk to move, Type numbers, then enter to subtract and esc to return"
        //         }
        //     },
        // };
        // Paragraph::new(description).centered().render(area, buf);
        Paragraph::new("TODO").centered().render(area, buf);
    }

    fn render_list(&mut self, area: Rect, buf: &mut Buffer) {
        let block = Block::new()
            .title(Line::raw("Counters").centered())
            .borders(Borders::all())
            .border_set(symbols::border::ROUNDED);

        // Iterate through all elements in the `items` and stylize them.
        let items: Vec<ListItem> = self
            .history
            .calculate_counter_results()
            .iter()
            .map(|(name, value)| {
                ListItem::new(Line::styled(format!("{}: {}", name, value), Color::White))
            })
            .collect();

        // Create a List from all list items and highlight the currently selected one
        let list = List::new(items)
            .block(block)
            //.highlight_style(SELECTED_STYLE)
            .highlight_symbol(">")
            .highlight_spacing(HighlightSpacing::Always);

        // We need to disambiguate this trait method as both `Widget` and `StatefulWidget` share the
        // same method name `render`.
        StatefulWidget::render(list, area, buf, &mut self.counter_list_state);
    }

    pub(crate) fn render_history(&mut self, area: Rect, buf: &mut Buffer) {
        let block = Block::new()
            .title(Line::raw("History").centered())
            .borders(Borders::all())
            .border_set(symbols::border::ROUNDED);

        // Iterate through all elements in the `items` and stylize them.
        let items: Vec<ListItem> = self
            .history
            .0
            .iter()
            .map(|entry| ListItem::from(entry))
            .collect();

        // Create a List from all list items and highlight the currently selected one
        let list = List::new(items)
            .block(block)
            //.highlight_style(SELECTED_STYLE)
            .highlight_symbol(">")
            .highlight_spacing(HighlightSpacing::Always);

        // We need to disambiguate this trait method as both `Widget` and `StatefulWidget` share the
        // same method name `render`.
        StatefulWidget::render(list, area, buf, &mut self.history_list_state);
    }

    fn render_input(&mut self, area: Rect, buf: &mut Buffer) {
        match &self.input_mode {
            InputMode::Normal(_) => {}
            InputMode::NewCounter(input) => {
                let block = Block::new()
                    .title(Line::raw("New Counter").centered())
                    .borders(Borders::all())
                    .border_set(symbols::border::ROUNDED);

                Paragraph::new(input.value())
                    .centered()
                    .block(block)
                    .render(area, buf);
            }
            InputMode::Adding(input, sign) => {
                let block = Block::new()
                    .title(
                        Line::raw(match sign {
                            AddingModeSign::Positive => "Adding",
                            AddingModeSign::Negative => "Subtracting",
                        })
                        .centered(),
                    )
                    .borders(Borders::all())
                    .border_set(symbols::border::ROUNDED);

                Paragraph::new(input.value())
                    .centered()
                    .block(block)
                    .render(area, buf);
            }
        }
    }
}

impl Widget for &mut App {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let [main_area, footer_area] =
            Layout::vertical([Constraint::Fill(1), Constraint::Length(1)]).areas(area);

        let [counters_area, history_area] =
            Layout::horizontal([Constraint::Fill(1), Constraint::Fill(1)]).areas(main_area);

        let [adding_area, list_area] =
            Layout::vertical([Constraint::Length(3), Constraint::Fill(1)]).areas(main_area);

        match &self.input_mode {
            InputMode::Normal(normal_focus) => {
                self.render_list(counters_area, buf);
                self.render_history(history_area, buf);
            }
            InputMode::NewCounter(_) => {
                self.render_input(adding_area, buf);
                self.render_list(list_area, buf);
            }
            InputMode::Adding(_, _) => {
                self.render_input(adding_area, buf);
                self.render_list(list_area, buf);
            }
        }

        self.render_footer(footer_area, buf);
    }
}
