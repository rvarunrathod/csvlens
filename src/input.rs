use crate::app::WrapMode;
use crate::command::{self, ColonCommand, CompletionResult};
use crate::common::InputMode;
use crate::history::BufferHistoryContainer;
use crate::util::events::{CsvlensEvent, CsvlensEvents};
use crate::watch::FileWatcher;
use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use tui_input::Input;
use tui_input::backend::crossterm::EventHandler;

pub enum Control {
    ScrollUp,
    ScrollDown,
    ScrollLeft,
    ScrollRight,
    ScrollTop,
    ScrollBottom,
    ScrollPageUp,
    ScrollPageDown,
    ScrollHalfPageUp,
    ScrollHalfPageDown,
    ScrollPageLeft,
    ScrollPageRight,
    ScrollLeftMost,
    ScrollRightMost,
    ScrollTo(usize),
    ScrollToNextFound,
    ScrollToPrevFound,
    IncreaseWidth,
    DecreaseWidth,
    Find(String),
    FindLikeCell,
    Filter(String),
    FilterColumns(String),
    FilterLikeCell,
    FreezeColumns(usize),
    /// Parsed `:` command (see [`crate::command`]).
    Command(ColonCommand),
    /// Jump to column by fuzzy-matched name (`gc` prompt).
    GotoColumn(String),
    /// Toggle hide on selected/current column (`zh` / `:hide`).
    ToggleHideColumn,
    /// Show all columns (`za` / `zr` / `:show`).
    ShowAllColumns,
    /// Hide all columns except selected/current (`zo` / `:only`).
    HideAllExceptSelectedColumn,
    Quit,
    BufferContent(Input),
    BufferReset,
    /// Clear the status-line input buffer without resetting filters.
    ClearInputBuffer,
    Select,
    CopySelection,
    SelectMarks,
    ToggleSelectionType,
    ToggleLineWrap(WrapMode),
    ToggleMark,
    ResetMarks,
    ToggleSort,
    ToggleNaturalSort,
    Reset,
    Help,
    UnknownOption(String),
    UserError(String),
    FileChanged,
    Nothing,
}

impl Control {
    fn empty_buffer() -> Control {
        Control::BufferContent("".into())
    }
}

enum BufferState {
    Active(Input),
    Inactive,
}

pub struct InputHandler {
    events: CsvlensEvents,
    mode: InputMode,
    buffer_state: BufferState,
    buffer_history_container: BufferHistoryContainer,
    /// Column headers for Tab completion in command / filter modes.
    completion_columns: Vec<String>,
    /// Display labels for the active completion set (`completion_index` selects one).
    pub completion_candidates: Vec<String>,
    /// Full buffer lines for each candidate (parallel to `completion_candidates`).
    completion_lines: Vec<String>,
    /// Selected candidate index, or `-1` when the picker is closed.
    pub completion_index: isize,
}

impl InputHandler {
    pub fn new(file_watcher: Option<FileWatcher>) -> InputHandler {
        InputHandler {
            events: CsvlensEvents::new(file_watcher),
            mode: InputMode::Default,
            buffer_state: BufferState::Inactive,
            buffer_history_container: BufferHistoryContainer::new(),
            completion_columns: Vec::new(),
            completion_candidates: Vec::new(),
            completion_lines: Vec::new(),
            completion_index: -1,
        }
    }

    /// Update headers used for column-name completion (call when data/headers change).
    pub fn set_completion_columns(&mut self, columns: Vec<String>) {
        self.completion_columns = columns;
    }

    fn completion_open(&self) -> bool {
        !self.completion_lines.is_empty() && self.completion_index >= 0
    }

    pub fn clear_completion_ui(&mut self) {
        self.completion_candidates.clear();
        self.completion_lines.clear();
        self.completion_index = -1;
    }

    pub fn next(&mut self) -> Control {
        match self.events.next().unwrap() {
            CsvlensEvent::Input(key) => self.handle_key(key),
            CsvlensEvent::FileChanged => Control::FileChanged,
            CsvlensEvent::Tick => Control::Nothing,
        }
    }

    fn handle_key(&mut self, mut key: KeyEvent) -> Control {
        /*
        The shift key modifier is not consistent across platforms.

        For upper case alphabets, e.g. 'A'

        Unix: Char("A") + SHIFT
        Windows: Char("A") + SHIFT

        For non-alphabets, e.g. '>'

        Unix: Char(">") + NULL
        Windows: Char(">") + SHIFT

        But the key event handling below assumes that the shift key modifier is only added for
        alphabets. To satisfy the assumption, the following ensures that the presence or absence
        of shift modifier is consistent across platforms.

        Idea borrowed from: https://github.com/sxyazi/yazi/pull/174
        */
        let platform_consistent_shift = match (key.code, key.modifiers) {
            (KeyCode::Char(c), _) => c.is_ascii_uppercase(),
            (_, m) => m.contains(KeyModifiers::SHIFT),
        };
        if platform_consistent_shift {
            key.modifiers.insert(KeyModifiers::SHIFT);
        } else {
            key.modifiers.remove(KeyModifiers::SHIFT);
        }
        if self.is_help_mode() {
            self.handler_help(key)
        } else if self.is_input_buffering() {
            self.handler_buffering(key)
        } else {
            self.handler_default(key)
        }
    }

    fn handler_default(&mut self, key_event: KeyEvent) -> Control {
        match key_event.modifiers {
            KeyModifiers::NONE => match key_event.code {
                KeyCode::Char('q') => Control::Quit,
                KeyCode::Char('j') | KeyCode::Down => Control::ScrollDown,
                KeyCode::Char('k') | KeyCode::Up => Control::ScrollUp,
                KeyCode::Char('l') | KeyCode::Right => Control::ScrollRight,
                KeyCode::Char('h') | KeyCode::Left => Control::ScrollLeft,
                KeyCode::Home => Control::ScrollTop,
                KeyCode::End => Control::ScrollBottom,
                KeyCode::Char('g') => {
                    self.init_buffer(InputMode::GoPrefix);
                    Control::empty_buffer()
                }
                KeyCode::Char('z') => {
                    self.init_buffer(InputMode::ColumnVisibilityPrefix);
                    Control::empty_buffer()
                }
                KeyCode::Char('n') => Control::ScrollToNextFound,
                KeyCode::PageDown => Control::ScrollPageDown,
                KeyCode::PageUp => Control::ScrollPageUp,
                KeyCode::Char('d') => Control::ScrollHalfPageDown,
                KeyCode::Char('u') => Control::ScrollHalfPageUp,
                KeyCode::Char(x) if "0123456789".contains(x.to_string().as_str()) => {
                    self.buffer_state = BufferState::Active(Input::new(x.to_string()));
                    self.mode = InputMode::GotoLine;
                    Control::BufferContent(Input::new(x.to_string()))
                }
                KeyCode::Char('/') => {
                    self.init_buffer(InputMode::Find);
                    Control::empty_buffer()
                }
                KeyCode::Char('&') => {
                    self.init_buffer(InputMode::Filter);
                    Control::empty_buffer()
                }
                KeyCode::Char(':') => {
                    self.init_buffer(InputMode::Command);
                    Control::empty_buffer()
                }
                KeyCode::Char('*') => {
                    self.init_buffer(InputMode::FilterColumns);
                    Control::empty_buffer()
                }
                KeyCode::Char('-') => {
                    self.init_buffer(InputMode::Option);
                    Control::empty_buffer()
                }
                KeyCode::Char('f') => {
                    self.init_buffer(InputMode::FreezeColumns);
                    Control::empty_buffer()
                }
                KeyCode::Enter => Control::Select,
                KeyCode::Tab => Control::ToggleSelectionType,
                KeyCode::Char('>') => Control::IncreaseWidth,
                KeyCode::Char('<') => Control::DecreaseWidth,
                KeyCode::Char('r') => Control::Reset,
                KeyCode::Char('?') => Control::Help,
                KeyCode::Char('#') => Control::FindLikeCell,
                KeyCode::Char('@') => Control::FilterLikeCell,
                KeyCode::Char('y') => Control::CopySelection,
                KeyCode::Char('m') => Control::ToggleMark,
                _ => Control::Nothing,
            },
            KeyModifiers::SHIFT => match key_event.code {
                KeyCode::Char('G') | KeyCode::End => Control::ScrollBottom,
                KeyCode::Char('N') => Control::ScrollToPrevFound,
                KeyCode::Char('H') => Control::Help,
                KeyCode::Char('J') | KeyCode::Down => Control::ToggleSort,
                KeyCode::Char('M') => Control::ResetMarks,
                _ => Control::Nothing,
            },
            KeyModifiers::CONTROL => match key_event.code {
                KeyCode::Char('f') => Control::ScrollPageDown,
                KeyCode::Char('b') => Control::ScrollPageUp,
                KeyCode::Char('d') => Control::ScrollHalfPageDown,
                KeyCode::Char('u') => Control::ScrollHalfPageUp,
                KeyCode::Char('h') => Control::ScrollPageLeft,
                KeyCode::Char('l') => Control::ScrollPageRight,
                KeyCode::Left => Control::ScrollLeftMost,
                KeyCode::Right => Control::ScrollRightMost,
                KeyCode::Char('j') => Control::ToggleNaturalSort,
                KeyCode::Char('e') => Control::SelectMarks,
                _ => Control::Nothing,
            },
            _ => Control::Nothing,
        }
    }

    fn handler_buffering(&mut self, key_event: KeyEvent) -> Control {
        if !matches!(self.buffer_state, BufferState::Active(_)) {
            return Control::Nothing;
        }
        if self.mode == InputMode::Option {
            return self.handler_buffering_option_mode(key_event);
        }
        if self.mode == InputMode::GoPrefix {
            return self.handler_go_prefix(key_event);
        }
        if self.mode == InputMode::ColumnVisibilityPrefix {
            return self.handler_column_visibility_prefix(key_event);
        }
        let completion_open = self.completion_open();
        let in_complete_mode = matches!(
            self.mode,
            InputMode::Command | InputMode::Filter | InputMode::Find | InputMode::GotoColumn
        );

        match key_event.code {
            KeyCode::Esc if completion_open => {
                // First Esc only dismisses the picker (fzf-like).
                let value = self.active_buffer_value();
                let cursor = value.len();
                self.clear_completion_ui();
                let input = Input::new(value).with_cursor(cursor);
                self.buffer_state = BufferState::Active(input.clone());
                Control::BufferContent(input)
            }
            KeyCode::Esc => {
                self.clear_completion_ui();
                self.reset_buffer();
                Control::BufferReset
            }
            // Open or cycle completion picker
            KeyCode::Tab | KeyCode::BackTab if in_complete_mode => {
                let reverse = matches!(key_event.code, KeyCode::BackTab)
                    || key_event.modifiers.contains(KeyModifiers::SHIFT);
                self.apply_tab_completion(reverse)
            }
            // Navigate picker without touching command history
            KeyCode::Up | KeyCode::Down if completion_open => {
                let reverse = matches!(key_event.code, KeyCode::Up);
                self.cycle_completion(reverse)
            }
            KeyCode::Char('g' | 'G') | KeyCode::Enter if self.mode == InputMode::GotoLine => {
                let value = self.active_buffer_value();
                self.buffer_history_container.set(self.mode, &value);
                let goto_line = value.parse::<usize>().ok();
                let res = if let Some(n) = goto_line {
                    Control::ScrollTo(n)
                } else {
                    Control::BufferReset
                };
                self.reset_buffer();
                res
            }
            // Enter with picker open: accept selection into the line, keep editing
            KeyCode::Enter if completion_open => {
                let line = self
                    .completion_lines
                    .get(self.completion_index as usize)
                    .cloned()
                    .unwrap_or_else(|| self.active_buffer_value());
                self.clear_completion_ui();
                self.apply_completion_line(&line)
            }
            KeyCode::Up => {
                self.clear_completion_ui();
                let mode = match self.mode {
                    InputMode::Filter => InputMode::Find,
                    InputMode::Command => InputMode::Command,
                    _ => self.mode,
                };
                if let Some(buf) = self.buffer_history_container.prev(mode) {
                    self.buffer_state = BufferState::Active(Input::new(buf.clone()));
                    Control::BufferContent(Input::new(buf))
                } else {
                    Control::Nothing
                }
            }
            KeyCode::Down => {
                self.clear_completion_ui();
                let mode = match self.mode {
                    InputMode::Filter => InputMode::Find,
                    InputMode::Command => InputMode::Command,
                    _ => self.mode,
                };
                if let Some(buf) = self.buffer_history_container.next(mode) {
                    self.buffer_state = BufferState::Active(Input::new(buf.clone()));
                    Control::BufferContent(Input::new(buf))
                } else {
                    self.buffer_state = BufferState::Active(Input::default());
                    Control::BufferContent(Input::default())
                }
            }
            KeyCode::Enter => {
                self.clear_completion_ui();
                let value = self.active_buffer_value();
                let mode = self.mode;
                let control = if value.is_empty() {
                    Control::BufferReset
                } else if mode == InputMode::Find {
                    if command::looks_like_expression(&value) {
                        match command::parse_filter_expr(&value) {
                            Ok(expr) => Control::Command(ColonCommand::Find(expr)),
                            Err(_) => Control::Find(value.clone()),
                        }
                    } else {
                        Control::Find(value.clone())
                    }
                } else if mode == InputMode::Filter {
                    if command::looks_like_expression(&value) {
                        match command::parse_filter_expr(&value) {
                            Ok(expr) => Control::Command(ColonCommand::Filter(expr)),
                            Err(_) => Control::Filter(value.clone()),
                        }
                    } else {
                        Control::Filter(value.clone())
                    }
                } else if mode == InputMode::FilterColumns {
                    Control::FilterColumns(value.clone())
                } else if mode == InputMode::GotoColumn {
                    Control::GotoColumn(value.clone())
                } else if mode == InputMode::Command {
                    match command::parse_colon_command(&value) {
                        Ok(cmd) => Control::Command(cmd),
                        Err(e) => Control::UserError(e),
                    }
                } else {
                    Control::BufferReset
                };
                if mode == InputMode::Filter {
                    self.buffer_history_container.set(InputMode::Find, &value);
                } else {
                    self.buffer_history_container.set(mode, &value);
                }
                if !matches!(control, Control::UserError(_)) {
                    self.reset_buffer();
                }
                control
            }
            _ => {
                let is_edit = !matches!(
                    key_event.code,
                    KeyCode::Left | KeyCode::Right | KeyCode::Home | KeyCode::End
                );
                if is_edit {
                    self.clear_completion_ui();
                }
                let BufferState::Active(ref mut input) = self.buffer_state else {
                    return Control::Nothing;
                };
                if input.handle_event(&Event::Key(key_event)).is_some() {
                    let control = if self.mode == InputMode::FreezeColumns {
                        let control = if let Ok(n) = input.value().parse::<usize>() {
                            Control::FreezeColumns(n)
                        } else {
                            Control::UserError(format!("Invalid number: {}", input.value()))
                        };
                        self.reset_buffer();
                        control
                    } else {
                        Control::BufferContent(input.clone())
                    };
                    return control;
                }
                Control::Nothing
            }
        }
    }

    fn active_buffer_value(&self) -> String {
        match &self.buffer_state {
            BufferState::Active(input) => input.value().to_string(),
            BufferState::Inactive => String::new(),
        }
    }

    /// Open the completion picker (first Tab) or move the selection (later Tab / arrows).
    /// The command line is updated live to preview the selection (fzf-style).
    fn apply_tab_completion(&mut self, reverse: bool) -> Control {
        if !matches!(self.buffer_state, BufferState::Active(_)) {
            return Control::Nothing;
        }

        if self.completion_open() {
            return self.cycle_completion(reverse);
        }

        let current = self.active_buffer_value();
        let line_for_complete = match self.mode {
            InputMode::Command => current,
            InputMode::GotoColumn => {
                // Reuse `:gc` column completion.
                if current.is_empty() {
                    "gc ".to_string()
                } else {
                    format!("gc {current}")
                }
            }
            InputMode::Filter | InputMode::Find => {
                if current.is_empty() {
                    "filter ".to_string()
                } else {
                    format!("filter {current}")
                }
            }
            _ => current,
        };
        let CompletionResult {
            line,
            candidates,
            lines,
            index,
        } = command::complete_command_line(&line_for_complete, &self.completion_columns, 0);
        if lines.is_empty() {
            return Control::Nothing;
        }
        self.completion_candidates = candidates;
        self.completion_lines = lines;
        self.completion_index = index;
        self.apply_completion_line(&line)
    }

    fn cycle_completion(&mut self, reverse: bool) -> Control {
        if self.completion_lines.is_empty() {
            return Control::Nothing;
        }
        let n = self.completion_lines.len();
        let idx = if reverse {
            if self.completion_index <= 0 {
                n - 1
            } else {
                self.completion_index as usize - 1
            }
        } else {
            (self.completion_index as usize + 1) % n
        };
        self.completion_index = idx as isize;
        let line = self.completion_lines[idx].clone();
        self.apply_completion_line(&line)
    }

    fn apply_completion_line(&mut self, line: &str) -> Control {
        let new_value = match self.mode {
            InputMode::Filter | InputMode::Find => {
                line.strip_prefix("filter ").unwrap_or(line).to_string()
            }
            InputMode::GotoColumn => line.strip_prefix("gc ").unwrap_or(line).to_string(),
            _ => line.to_string(),
        };
        let cursor = new_value.len();
        let new_input = Input::new(new_value).with_cursor(cursor);
        self.buffer_state = BufferState::Active(new_input.clone());
        Control::BufferContent(new_input)
    }

    fn handler_go_prefix(&mut self, key_event: KeyEvent) -> Control {
        match key_event.code {
            KeyCode::Esc | KeyCode::Backspace => {
                self.reset_buffer();
                Control::ClearInputBuffer
            }
            // `gc` — go to column by name
            KeyCode::Char('c') => {
                self.init_buffer(InputMode::GotoColumn);
                Control::empty_buffer()
            }
            // `gg` or Enter — go to top (vim-style)
            KeyCode::Char('g') | KeyCode::Enter => {
                self.reset_buffer();
                Control::ScrollTop
            }
            _ => {
                self.reset_buffer();
                Control::ScrollTop
            }
        }
    }

    fn handler_column_visibility_prefix(&mut self, key_event: KeyEvent) -> Control {
        match key_event.code {
            KeyCode::Esc | KeyCode::Backspace => {
                self.reset_buffer();
                Control::ClearInputBuffer
            }
            // `zh` — toggle hide on selected/current column
            KeyCode::Char('h') => {
                self.reset_buffer();
                Control::ToggleHideColumn
            }
            // `za` / `zr` — show all columns
            KeyCode::Char('a') | KeyCode::Char('r') => {
                self.reset_buffer();
                Control::ShowAllColumns
            }
            // `zo` — hide all except selected/current column
            KeyCode::Char('o') => {
                self.reset_buffer();
                Control::HideAllExceptSelectedColumn
            }
            KeyCode::Char(x) => {
                self.reset_buffer();
                Control::UnknownOption(format!("z{x}"))
            }
            _ => {
                self.reset_buffer();
                Control::ClearInputBuffer
            }
        }
    }

    fn handler_buffering_option_mode(&mut self, key_event: KeyEvent) -> Control {
        match key_event.code {
            KeyCode::Esc | KeyCode::Backspace | KeyCode::Enter => {
                self.reset_buffer();
                Control::BufferReset
            }
            KeyCode::Char('S') => {
                self.reset_buffer();
                Control::ToggleLineWrap(WrapMode::Chars)
            }
            KeyCode::Char('W') | KeyCode::Char('w') => {
                self.reset_buffer();
                Control::ToggleLineWrap(WrapMode::Words)
            }
            KeyCode::Char(x) => {
                self.reset_buffer();
                Control::UnknownOption(x.to_string())
            }
            _ => Control::Nothing,
        }
    }

    fn handler_help(&mut self, key_event: KeyEvent) -> Control {
        match key_event.code {
            KeyCode::Char('q') | KeyCode::Esc => Control::Quit,
            KeyCode::Char('j') | KeyCode::Down => Control::ScrollDown,
            KeyCode::Char('k') | KeyCode::Up => Control::ScrollUp,
            _ => Control::Nothing,
        }
    }

    fn is_input_buffering(&self) -> bool {
        matches!(self.buffer_state, BufferState::Active(_))
    }

    fn init_buffer(&mut self, mode: InputMode) {
        self.buffer_state = BufferState::Active(Input::default());
        self.mode = mode;
    }

    fn reset_buffer(&mut self) {
        self.buffer_state = BufferState::Inactive;
        self.buffer_history_container.reset_cursors();
        self.mode = InputMode::Default;
    }

    pub fn mode(&self) -> InputMode {
        self.mode
    }

    pub fn enter_help_mode(&mut self) {
        self.mode = InputMode::Help;
    }

    pub fn exit_help_mode(&mut self) {
        self.mode = InputMode::Default;
    }

    fn is_help_mode(&mut self) -> bool {
        self.mode == InputMode::Help
    }
}
