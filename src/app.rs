use std::io;

use ratatui::crossterm::event;
use ratatui::crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind};
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, HighlightSpacing, List, ListItem, ListState, Paragraph};
use ratatui::Terminal;
use tui_input::backend::crossterm::EventHandler;
use tui_input::Input;

enum InputMode {
    Normal,
    Adding(Input),
}

struct CounterList {
    counters: Vec<Counter>,
    state: ListState,
}

impl Default for CounterList {
    fn default() -> Self {
        Self {
            counters: vec![],
            state: Default::default(),
        }
    }
}

pub(crate) struct App {
    counter_list: CounterList,
    input_mode: InputMode,
    should_exit: bool,
}

impl Default for App {
    fn default() -> Self {
        App {
            counter_list: CounterList::default(),
            input_mode: InputMode::Normal,
            should_exit: false,
        }
    }
}

impl App {
    pub(crate) fn run(&mut self, mut terminal: Terminal<impl Backend>) -> io::Result<()> {
        while !self.should_exit {
            //terminal.clear()?;
            terminal.draw(|f| f.render_widget(&mut *self, f.size()))?;
            if let Event::Key(key) = event::read()? {
                self.handle_key(key);
            };
        }
        Ok(())
    }

    fn handle_key(&mut self, key: KeyEvent) {
        if key.kind != KeyEventKind::Press {
            return;
        }
        match &mut self.input_mode {
            InputMode::Normal => match key.code {
                KeyCode::Up | KeyCode::Char('k') => self.counter_list.state.select_previous(),
                KeyCode::Down | KeyCode::Char('j') => self.counter_list.state.select_next(),
                KeyCode::Right | KeyCode::Char('a') => match self.counter_list.state.selected() {
                    Some(index) => match self.counter_list.counters.get_mut(index) {
                        Some(counter) => counter.count += 1,
                        None => {}
                    },
                    None => {}
                },
                KeyCode::Left | KeyCode::Char('s') => match self.counter_list.state.selected() {
                    Some(index) => match self.counter_list.counters.get_mut(index) {
                        Some(counter) => counter.count -= 1,
                        None => {}
                    },
                    None => {}
                },
                KeyCode::Char('q') => self.should_exit = true,
                KeyCode::Char('n') => self.input_mode = InputMode::Adding(Input::default()),
                KeyCode::Char('d') => match self.counter_list.state.selected() {
                    Some(index) => {
                        self.counter_list.counters.remove(index);
                    }
                    None => {}
                },
                KeyCode::Esc => self.counter_list.state.select(None),
                _ => {}
            },
            InputMode::Adding(input) => match key.code {
                KeyCode::Esc => self.input_mode = InputMode::Normal,
                KeyCode::Enter => {
                    self.counter_list.counters.push(Counter::new(input.value()));
                    input.reset();
                }
                _ => {
                    input.handle_event(&Event::Key(key));
                }
            },
        }
    }

    fn render_footer(&self, area: Rect, buf: &mut Buffer) {
        let description = match self.input_mode {
            InputMode::Normal => {
                "Use ↓↑/jk to move, d to delete, ←→/as to change the counter, and q to exit."
            }
            InputMode::Adding(_) => "Type a new counter name. Use enter to add and esc to return.",
        };
        Paragraph::new(description).centered().render(area, buf);
    }

    fn render_list(&mut self, area: Rect, buf: &mut Buffer) {
        let block = Block::new()
            .title(Line::raw("Counters").centered())
            .borders(Borders::all())
            .border_set(symbols::border::ROUNDED);

        // Iterate through all elements in the `items` and stylize them.
        let items: Vec<ListItem> = self
            .counter_list
            .counters
            .iter()
            .map(|counter| ListItem::from(counter))
            .collect();

        // Create a List from all list items and highlight the currently selected one
        let list = List::new(items)
            .block(block)
            //.highlight_style(SELECTED_STYLE)
            .highlight_symbol(">")
            .highlight_spacing(HighlightSpacing::Always);

        // We need to disambiguate this trait method as both `Widget` and `StatefulWidget` share the
        // same method name `render`.
        StatefulWidget::render(list, area, buf, &mut self.counter_list.state);
    }

    fn render_input(&mut self, area: Rect, buf: &mut Buffer) {
        let block = Block::new()
            .title(Line::raw("New Counter").centered())
            .borders(Borders::all())
            .border_set(symbols::border::ROUNDED);

        match &self.input_mode {
            InputMode::Normal => {}
            InputMode::Adding(input) => {
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

        let [adding_area, list_area] =
            Layout::vertical([Constraint::Length(3), Constraint::Fill(1)]).areas(main_area);

        match self.input_mode {
            InputMode::Normal => {
                self.render_list(main_area, buf);
            }
            InputMode::Adding(_) => {
                self.render_input(adding_area, buf);
                self.render_list(list_area, buf);
            }
        }

        self.render_footer(footer_area, buf);
    }
}

struct Counter {
    name: String,
    count: i64,
}

impl Counter {
    fn new(name: &str) -> Self {
        Self {
            name: name.to_owned(),
            count: 0,
        }
    }
}

impl From<&Counter> for ListItem<'_> {
    fn from(value: &Counter) -> Self {
        let line = Line::styled(format!("{}: {}", value.name, value.count), Color::White);

        ListItem::new(line)
    }
}
