use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, StatefulWidget, Widget, Wrap},
};

/// Accent colors for the help overlay (independent of the data theme so help
/// stays readable on both light and dark terminals).
struct HelpPalette {
    title: Style,
    section: Style,
    key: Style,
    sep: Style,
    desc: Style,
    hint: Style,
    example: Style,
    border: Style,
    dim: Style,
}

impl HelpPalette {
    fn dark() -> Self {
        Self {
            title: Style::default()
                .fg(Color::Rgb(250, 189, 47))
                .add_modifier(Modifier::BOLD),
            section: Style::default()
                .fg(Color::Rgb(131, 165, 152))
                .add_modifier(Modifier::BOLD),
            key: Style::default()
                .fg(Color::Rgb(254, 128, 25))
                .add_modifier(Modifier::BOLD),
            sep: Style::default().fg(Color::Rgb(80, 73, 69)),
            desc: Style::default().fg(Color::Rgb(235, 219, 178)),
            hint: Style::default().fg(Color::Rgb(168, 153, 132)),
            example: Style::default().fg(Color::Rgb(142, 192, 124)),
            border: Style::default().fg(Color::Rgb(80, 73, 69)),
            dim: Style::default().fg(Color::Rgb(102, 92, 84)),
        }
    }
}

fn help_lines(p: &HelpPalette) -> Vec<Line<'static>> {
    let mut lines: Vec<Line<'static>> = Vec::new();

    let blank = || Line::from("");
    let section = |title: &str| {
        Line::from(vec![
            Span::styled("  ▸ ", p.section),
            Span::styled(title.to_string(), p.section),
            Span::styled(
                format!(" {}", "─".repeat(48usize.saturating_sub(title.len()))),
                p.dim,
            ),
        ])
    };
    let binding = |key: &str, desc: &str| {
        // Fixed key column width for alignment
        const KEY_W: usize = 28;
        let key_pad = if key.len() < KEY_W {
            format!("{key:<KEY_W$}")
        } else {
            key.to_string()
        };
        Line::from(vec![
            Span::raw("  "),
            Span::styled(key_pad, p.key),
            Span::styled("│ ", p.sep),
            Span::styled(desc.to_string(), p.desc),
        ])
    };
    let note = |text: &str| Line::from(vec![Span::raw("  "), Span::styled(text.to_string(), p.hint)]);
    let example = |text: &str| {
        Line::from(vec![
            Span::raw("    "),
            Span::styled(text.to_string(), p.example),
        ])
    };

    lines.push(Line::from(vec![
        Span::styled("  csvlens", p.title),
        Span::styled("  —  interactive CSV viewer", p.hint),
    ]));
    lines.push(note("Press q or Esc to close this help ·  j/k or ↑/↓ to scroll"));
    lines.push(blank());

    lines.push(section("Moving"));
    lines.push(binding("hjkl  /  ←↓↑→", "Scroll one row or column"));
    lines.push(binding("Ctrl+f  /  PgDn", "Scroll one window down"));
    lines.push(binding("Ctrl+b  /  PgUp", "Scroll one window up"));
    lines.push(binding("Ctrl+d  /  d", "Scroll half window down"));
    lines.push(binding("Ctrl+u  /  u", "Scroll half window up"));
    lines.push(binding("Ctrl+h / Ctrl+l", "Scroll one window left / right"));
    lines.push(binding("Ctrl+← / Ctrl+→", "Jump to first / last column"));
    lines.push(binding("g  /  Home", "Go to top"));
    lines.push(binding("G  /  End", "Go to bottom"));
    lines.push(binding("<n>G", "Go to line n"));
    lines.push(blank());

    lines.push(section("Search & filter"));
    lines.push(binding("/<regex>", "Find and highlight matches"));
    lines.push(binding("n / N", "Next / previous match"));
    lines.push(binding("&<regex>", "Filter rows (regex on any cell)"));
    lines.push(binding("*<regex>", "Filter columns by header regex"));
    lines.push(binding("Tab  (in :  &  /)", "Open completion picker (fzf-style)"));
    lines.push(note("Picker: ↑↓ or Tab cycle · Enter accept · Esc dismiss"));
    lines.push(blank());

    lines.push(section("Colon commands  (:)"));
    lines.push(binding(":filter <expr>", "Column-scoped filter (AND clauses)"));
    lines.push(binding(":find <expr>", "Column-scoped find / highlight"));
    lines.push(binding(":columns a,b,\"Name\"", "Show only these columns (or regex)"));
    lines.push(binding(":sort [+|-]<col>", "Sort ascending (+) or descending (-)"));
    lines.push(binding(":goto <n>", "Jump to line n"));
    lines.push(binding(":theme <name>", "Switch color theme"));
    lines.push(binding(":clear", "Clear filters, find, columns, sort"));
    lines.push(binding(":help  /  :q", "This help / quit"));
    lines.push(blank());
    lines.push(note("Filter operators"));
    lines.push(example("=   !=   ~  (contains)   >   <   :empty   :null"));
    lines.push(note("Column names with spaces — use quotes"));
    lines.push(example(r#""First Name"=Ann    `Order Date`>2020"#));
    lines.push(note("Sugar: status:failed  ≡  status=failed"));
    lines.push(note("Examples"));
    lines.push(example(":filter status=error severity>3"));
    lines.push(example(r#":filter "First Name"~Al and email:empty"#));
    lines.push(example("&status!=ok age<30"));
    lines.push(blank());

    lines.push(section("Selection"));
    lines.push(binding("TAB", "Cycle row → column → cell selection"));
    lines.push(binding(">  /  <", "Widen / narrow selected column"));
    lines.push(binding("Shift+↓  /  J", "Sort by column (auto type)"));
    lines.push(binding("Ctrl+j", "Natural sort (file2 < file10)"));
    lines.push(binding("#  (cell mode)", "Find rows equal to selected cell"));
    lines.push(binding("@  (cell mode)", "Filter rows equal to selected cell"));
    lines.push(binding("y", "Copy selection to clipboard"));
    lines.push(binding("Enter  (cell mode)", "Print cell to stdout and exit"));
    lines.push(blank());

    lines.push(section("Other"));
    lines.push(binding("-S / -W", "Wrap by chars / words"));
    lines.push(binding("f<n>", "Freeze n columns from the left"));
    lines.push(binding("m / M", "Toggle mark on row / clear all marks"));
    lines.push(binding("Ctrl+e", "Print marked rows (with header) and exit"));
    lines.push(binding("r", "Reset view (filters, widths, …)"));
    lines.push(binding("H  /  ?", "Show this help"));
    lines.push(binding("q", "Quit"));
    lines.push(blank());
    lines.push(Line::from(vec![
        Span::raw("  "),
        Span::styled(
            "Tip: set a default theme in ~/.config/csvlens/config.toml",
            p.dim,
        ),
    ]));
    lines.push(example(r#"theme = "grovbox-dark""#));

    lines
}

pub struct HelpPage {}

pub struct HelpPageState {
    active: bool,
    offset: u16,
    render_complete: bool,
}

impl HelpPage {
    pub fn new() -> Self {
        HelpPage {}
    }
}

impl HelpPageState {
    pub fn new() -> Self {
        HelpPageState {
            active: false,
            offset: 0,
            render_complete: true,
        }
    }

    pub fn activate(&mut self) -> &Self {
        self.active = true;
        self.offset = 0;
        self
    }

    pub fn deactivate(&mut self) -> &Self {
        self.active = false;
        self.offset = 0;
        self
    }

    pub fn is_active(&self) -> bool {
        self.active
    }

    pub fn scroll_up(&mut self) -> &Self {
        if self.offset > 0 {
            self.offset -= 1;
        }
        self
    }

    pub fn scroll_down(&mut self) -> &Self {
        if !self.render_complete {
            self.offset += 1;
        }
        self
    }
}

impl StatefulWidget for HelpPage {
    type State = HelpPageState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        if area.area() == 0 {
            return;
        }

        // Dim the background slightly by clearing — keeps focus on the panel.
        Clear.render(area, buf);

        let p = HelpPalette::dark();
        let text = help_lines(&p);

        // Centered panel with margins for a card-like feel.
        let margin_x = area.width.saturating_div(12).clamp(1, 6);
        let margin_y = area.height.saturating_div(16).min(2);
        let panel = Rect::new(
            area.x.saturating_add(margin_x),
            area.y.saturating_add(margin_y),
            area.width.saturating_sub(margin_x.saturating_mul(2)),
            area.height.saturating_sub(margin_y.saturating_mul(2)),
        );

        // Inner fill so table content doesn't bleed through.
        for y in panel.y..panel.bottom() {
            for x in panel.x..panel.right() {
                if let Some(cell) = buf.cell_mut((x, y)) {
                    cell.set_symbol(" ");
                    cell.set_style(Style::default().bg(Color::Rgb(29, 32, 33)));
                }
            }
        }

        let block = Block::default()
            .title(Span::styled(" Help ", p.title))
            .title_bottom(Span::styled(" q/Esc close · j/k scroll ", p.dim))
            .borders(Borders::ALL)
            .border_style(p.border);

        let inner = block.inner(panel);
        block.render(panel, buf);

        // Visible line budget (account for block borders already via inner).
        let visible = inner.height;
        let total = text.len() as u16;
        let num_lines_to_be_rendered = total.saturating_sub(state.offset);
        state.render_complete = visible >= num_lines_to_be_rendered;

        let paragraph = Paragraph::new(text)
            .style(Style::default().bg(Color::Rgb(29, 32, 33)))
            .wrap(Wrap { trim: false })
            .scroll((state.offset, 0));

        paragraph.render(inner, buf);

        // Scroll indicator on the right edge when content overflows.
        if !state.render_complete || state.offset > 0 {
            let label = if state.offset == 0 {
                " ↓ more "
            } else if state.render_complete {
                " ↑ more "
            } else {
                " ↑↓ "
            };
            let lx = panel
                .right()
                .saturating_sub(label.len() as u16 + 1)
                .max(panel.x);
            buf.set_stringn(lx, panel.y, label, label.len(), p.hint);
        }
    }
}
