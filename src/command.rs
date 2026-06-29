//! Colon-command (`:`) parser, column filter expressions, and completion.
//!
//! ## Column names with spaces
//! Prefer quoting so completion and parsing stay unambiguous:
//! - Double quotes: `"First Name"=Alice`
//! - Single quotes: `'Order Date'>2020`
//! - Backticks: `` `user id`!=0 ``
//!
//! Unquoted names may use letters, digits, `_`, `.`, `-` (no spaces).
//! Tab completion always inserts the quoted form when a name needs quoting.
//!
//! ## Filter / find expression syntax
//! One or more clauses combined with AND (whitespace or explicit `and`):
//! ```text
//! status=error
//! status!=ok severity>3
//! "First Name"~Ann and age<30
//! email:empty
//! status:failed          # sugar for status=failed (colon without operator keyword)
//! ```
//!
//! Operators: `=`, `!=`, `~` / `contains`, `>`, `<`, `:empty` / `:null`, `:nempty`

use regex::Regex;
use std::fmt;

/// Top-level colon command after `:`.
#[derive(Debug, Clone, PartialEq)]
pub enum ColonCommand {
    /// `:filter <expr>` — keep rows matching expression (AND of clauses).
    Filter(FilterExpr),
    /// `:find <expr>` — highlight matches; supports same expression language.
    Find(FilterExpr),
    /// Legacy / whole-table regex still accepted as `:find /regex/` or plain text without ops.
    FindRegex(String),
    FilterRegex(String),
    /// `:columns <regex|a,b,c|"Name With Space">`
    Columns(String),
    /// `:sort [+-]column` — `+` ascending (default), `-` descending by name.
    Sort { column: String, descending: bool },
    /// `:goto <n>`
    Goto(usize),
    /// `:theme <name|path>`
    Theme(String),
    /// `:export <path>` — reserved; returns a friendly not-implemented for now if wired.
    Export(String),
    /// `:help` / `:h`
    Help,
    /// `:q` / `:quit`
    Quit,
    /// Clear find + row filter (`:filter` / `:find` with no args).
    ClearSearch,
    /// Clear search, column filter, and sort (`:clear`).
    Clear,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FilterExpr {
    pub clauses: Vec<FilterClause>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FilterClause {
    /// Raw column token as typed (may include quotes).
    pub column: String,
    pub op: FilterOp,
    /// Right-hand side; empty for Empty/NotEmpty.
    pub value: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilterOp {
    Eq,
    Ne,
    /// Substring / regex contains (case per app ignore_case).
    Contains,
    Gt,
    Lt,
    Empty,
    NotEmpty,
}

impl FilterOp {
    fn as_str(self) -> &'static str {
        match self {
            FilterOp::Eq => "=",
            FilterOp::Ne => "!=",
            FilterOp::Contains => "~",
            FilterOp::Gt => ">",
            FilterOp::Lt => "<",
            FilterOp::Empty => ":empty",
            FilterOp::NotEmpty => ":nempty",
        }
    }
}

/// Resolved predicate ready for the finder (column is origin/global index).
#[derive(Debug, Clone)]
pub struct ResolvedPredicate {
    pub column_index: usize,
    pub op: FilterOp,
    /// For Eq/Ne/Contains: regex applied to the cell.
    pub regex: Option<Regex>,
    /// For Gt/Lt: numeric threshold (non-numeric cells fail).
    pub number: Option<f64>,
}

impl FilterExpr {
    pub fn resolve(
        &self,
        headers: &[String],
        ignore_case: bool,
    ) -> Result<Vec<ResolvedPredicate>, String> {
        let mut out = Vec::with_capacity(self.clauses.len());
        for clause in &self.clauses {
            let name = unquote_column(&clause.column);
            let column_index = resolve_column_index(headers, &name)?;
            let (regex, number) = match clause.op {
                FilterOp::Empty | FilterOp::NotEmpty => (None, None),
                FilterOp::Gt | FilterOp::Lt => {
                    let n = clause
                        .value
                        .trim()
                        .parse::<f64>()
                        .map_err(|_| format!("expected number for {}{}, got '{}'", name, clause.op.as_str(), clause.value))?;
                    (None, Some(n))
                }
                FilterOp::Eq | FilterOp::Ne => {
                    let pat = format!("^{}$", regex::escape(&clause.value));
                    let re = compile_regex(&pat, ignore_case)?;
                    (Some(re), None)
                }
                FilterOp::Contains => {
                    let re = compile_regex(&regex::escape(&clause.value), ignore_case)?;
                    (Some(re), None)
                }
            };
            out.push(ResolvedPredicate {
                column_index,
                op: clause.op,
                regex,
                number,
            });
        }
        Ok(out)
    }

    pub fn display(&self) -> String {
        self.clauses
            .iter()
            .map(|c| {
                if matches!(c.op, FilterOp::Empty | FilterOp::NotEmpty) {
                    format!("{}{}", c.column, c.op.as_str())
                } else {
                    format!("{}{}{}", c.column, c.op.as_str(), c.value)
                }
            })
            .collect::<Vec<_>>()
            .join(" ")
    }
}

impl ResolvedPredicate {
    pub fn matches_cell(&self, field: &str) -> bool {
        let trimmed = field.trim();
        match self.op {
            FilterOp::Empty => is_empty_cell(trimmed),
            FilterOp::NotEmpty => !is_empty_cell(trimmed),
            FilterOp::Eq => self
                .regex
                .as_ref()
                .map(|r| r.is_match(field))
                .unwrap_or(false),
            FilterOp::Ne => !self
                .regex
                .as_ref()
                .map(|r| r.is_match(field))
                .unwrap_or(false),
            FilterOp::Contains => self
                .regex
                .as_ref()
                .map(|r| r.is_match(field))
                .unwrap_or(false),
            FilterOp::Gt => parse_number(field)
                .zip(self.number)
                .is_some_and(|(v, n)| v > n),
            FilterOp::Lt => parse_number(field)
                .zip(self.number)
                .is_some_and(|(v, n)| v < n),
        }
    }
}

fn is_empty_cell(trimmed: &str) -> bool {
    trimmed.is_empty()
        || trimmed.eq_ignore_ascii_case("null")
        || trimmed.eq_ignore_ascii_case("na")
        || trimmed == "\\N"
        || trimmed == "N/A"
}

fn parse_number(s: &str) -> Option<f64> {
    let t = s.trim().replace(',', "");
    t.parse::<f64>().ok()
}

fn compile_regex(pat: &str, ignore_case: bool) -> Result<Regex, String> {
    let pat = if ignore_case && pat == pat.to_lowercase() {
        format!("(?i){pat}")
    } else {
        pat.to_string()
    };
    Regex::new(&pat).map_err(|e| e.to_string())
}

fn resolve_column_index(headers: &[String], name: &str) -> Result<usize, String> {
    if let Some(i) = headers.iter().position(|h| h == name) {
        return Ok(i);
    }
    // Case-insensitive exact
    let lower = name.to_ascii_lowercase();
    let mut ci_matches: Vec<usize> = headers
        .iter()
        .enumerate()
        .filter(|(_, h)| h.to_ascii_lowercase() == lower)
        .map(|(i, _)| i)
        .collect();
    if ci_matches.len() == 1 {
        return Ok(ci_matches.remove(0));
    }
    // Unique prefix (case-insensitive)
    let mut prefix_matches: Vec<usize> = headers
        .iter()
        .enumerate()
        .filter(|(_, h)| h.to_ascii_lowercase().starts_with(&lower))
        .map(|(i, _)| i)
        .collect();
    if prefix_matches.len() == 1 {
        return Ok(prefix_matches.remove(0));
    }
    if prefix_matches.is_empty() && ci_matches.is_empty() {
        Err(format!("unknown column '{name}'"))
    } else {
        Err(format!(
            "ambiguous column '{name}' (matches multiple headers)"
        ))
    }
}

/// Parse a full colon command line **without** the leading `:`.
pub fn parse_colon_command(line: &str) -> Result<ColonCommand, String> {
    let line = line.trim();
    if line.is_empty() {
        return Err("empty command".into());
    }

    let (cmd, rest) = split_first_word(line);
    let cmd_lower = cmd.to_ascii_lowercase();

    match cmd_lower.as_str() {
        "q" | "quit" | "exit" => Ok(ColonCommand::Quit),
        "h" | "help" => Ok(ColonCommand::Help),
        "clear" => Ok(ColonCommand::Clear),
        "goto" | "g" => {
            let n = rest
                .trim()
                .parse::<usize>()
                .map_err(|_| format!("usage: :goto <line>, got '{rest}'"))?;
            Ok(ColonCommand::Goto(n))
        }
        "theme" => {
            if rest.trim().is_empty() {
                return Err("usage: :theme <name|path>".into());
            }
            Ok(ColonCommand::Theme(rest.trim().to_string()))
        }
        "export" | "w" | "write" => {
            if rest.trim().is_empty() {
                return Err("usage: :export <path>".into());
            }
            Ok(ColonCommand::Export(rest.trim().to_string()))
        }
        "columns" | "cols" => {
            if rest.trim().is_empty() {
                return Err("usage: :columns <regex|col1,col2,...>".into());
            }
            Ok(ColonCommand::Columns(rest.trim().to_string()))
        }
        "sort" => parse_sort(rest.trim()),
        "filter" | "v" => {
            let rest = rest.trim();
            if rest.is_empty() {
                return Ok(ColonCommand::ClearSearch);
            }
            if looks_like_expression(rest) {
                Ok(ColonCommand::Filter(parse_filter_expr(rest)?))
            } else {
                Ok(ColonCommand::FilterRegex(rest.to_string()))
            }
        }
        "find" | "search" => {
            let rest = rest.trim();
            if rest.is_empty() {
                return Ok(ColonCommand::ClearSearch);
            }
            if looks_like_expression(rest) {
                Ok(ColonCommand::Find(parse_filter_expr(rest)?))
            } else {
                Ok(ColonCommand::FindRegex(rest.to_string()))
            }
        }
        // Bare expression shortcut: `:status=error` when first token isn't a known command
        _ if looks_like_expression(line) => Ok(ColonCommand::Filter(parse_filter_expr(line)?)),
        _ => Err(format!(
            "unknown command '{cmd}'. Try :filter, :find, :columns, :sort, :goto, :theme, :export, :clear, :help"
        )),
    }
}

fn parse_sort(rest: &str) -> Result<ColonCommand, String> {
    if rest.is_empty() {
        return Err("usage: :sort [+|-]<column>".into());
    }
    let (descending, col) = if let Some(c) = rest.strip_prefix('-') {
        (true, c.trim())
    } else if let Some(c) = rest.strip_prefix('+') {
        (false, c.trim())
    } else {
        (false, rest)
    };
    if col.is_empty() {
        return Err("usage: :sort [+|-]<column>".into());
    }
    Ok(ColonCommand::Sort {
        column: col.to_string(),
        descending,
    })
}

pub fn looks_like_expression(s: &str) -> bool {
    // Has an operator-ish token or quoted column with op
    let s = s.trim();
    if s.contains("!=")
        || s.contains(">=")
        || s.contains("<=")
        || s.contains('=')
        || s.contains('>')
        || s.contains('<')
        || s.contains('~')
        || s.to_ascii_lowercase().contains(" contains ")
        || s.contains(":empty")
        || s.contains(":null")
        || s.contains(":nempty")
    {
        return true;
    }
    // status:failed sugar (single colon, not a command word)
    if let Some(i) = s.find(':') {
        let after = &s[i + 1..];
        return !after.is_empty()
            && !after.starts_with("//")
            && !s[..i].contains(' ');
    }
    false
}

/// Parse `col=val col2~x` AND-combined clauses.
pub fn parse_filter_expr(input: &str) -> Result<FilterExpr, String> {
    let mut clauses = Vec::new();
    let mut rest = input.trim();
    while !rest.is_empty() {
        // optional leading `and`
        let lower = rest.to_ascii_lowercase();
        if let Some(r) = lower.strip_prefix("and ") {
            rest = rest[rest.len() - r.len()..].trim_start();
            continue;
        }
        let (clause, consumed) = parse_one_clause(rest)?;
        clauses.push(clause);
        rest = rest[consumed..].trim_start();
        if rest.to_ascii_lowercase().starts_with("and ") {
            rest = rest[4..].trim_start();
        }
    }
    if clauses.is_empty() {
        return Err("empty filter expression".into());
    }
    Ok(FilterExpr { clauses })
}

fn parse_one_clause(input: &str) -> Result<(FilterClause, usize), String> {
    let (column, col_len) = parse_column_token(input)?;
    let after_col = &input[col_len..];
    let after_col_trim_start = after_col.len() - after_col.trim_start().len();
    let op_src = &input[col_len + after_col_trim_start..];

    // :empty / :null / :nempty / :value sugar
    if let Some(rest) = op_src.strip_prefix(':') {
        let (kw, kw_len) = take_ident(rest);
        let kw_lower = kw.to_ascii_lowercase();
        let total = col_len + after_col_trim_start + 1 + kw_len;
        return match kw_lower.as_str() {
            "empty" | "null" | "na" => Ok((
                FilterClause {
                    column,
                    op: FilterOp::Empty,
                    value: String::new(),
                },
                total,
            )),
            "nempty" | "notempty" | "nonempty" => Ok((
                FilterClause {
                    column,
                    op: FilterOp::NotEmpty,
                    value: String::new(),
                },
                total,
            )),
            // sugar: status:failed => status=failed
            other if !other.is_empty() => Ok((
                FilterClause {
                    column,
                    op: FilterOp::Eq,
                    value: kw.to_string(),
                },
                total,
            )),
            _ => Err("expected :empty, :null, or :value after column".into()),
        };
    }

    // word operator `contains`
    let op_lower = op_src.to_ascii_lowercase();
    if let Some(r) = op_lower.strip_prefix("contains") {
        let used = op_src.len() - r.len();
        let after_op = op_src[used..].trim_start();
        let ws = op_src[used..].len() - after_op.len();
        let (value, val_len) = parse_value_token(after_op)?;
        let total = col_len + after_col_trim_start + used + ws + val_len;
        return Ok((
            FilterClause {
                column,
                op: FilterOp::Contains,
                value,
            },
            total,
        ));
    }

    let (op, op_len) = if op_src.starts_with("!=") {
        (FilterOp::Ne, 2)
    } else if op_src.starts_with('=') {
        (FilterOp::Eq, 1)
    } else if op_src.starts_with('~') {
        (FilterOp::Contains, 1)
    } else if op_src.starts_with('>') {
        (FilterOp::Gt, 1)
    } else if op_src.starts_with('<') {
        (FilterOp::Lt, 1)
    } else {
        return Err(format!(
            "expected operator after column '{column}' (=, !=, ~, contains, >, <, :empty)"
        ));
    };

    let after_op = &op_src[op_len..];
    let after_op_trim = after_op.trim_start();
    let ws = after_op.len() - after_op_trim.len();
    let (value, val_len) = parse_value_token(after_op_trim)?;
    let total = col_len + after_col_trim_start + op_len + ws + val_len;
    Ok((
        FilterClause {
            column,
            op,
            value,
        },
        total,
    ))
}

/// Parse a column name token; returns (raw token including quotes, byte length consumed).
fn parse_column_token(input: &str) -> Result<(String, usize), String> {
    let bytes = input.as_bytes();
    if bytes.is_empty() {
        return Err("expected column name".into());
    }
    let quote = bytes[0];
    if quote == b'"' || quote == b'\'' || quote == b'`' {
        let mut i = 1;
        while i < bytes.len() {
            if bytes[i] == quote {
                let token = input[..i + 1].to_string();
                return Ok((token, i + 1));
            }
            // allow backslash-escape of the quote
            if bytes[i] == b'\\' && i + 1 < bytes.len() {
                i += 2;
                continue;
            }
            i += 1;
        }
        return Err("unterminated quoted column name".into());
    }
    // unquoted: stop at operator start or whitespace
    let mut i = 0;
    while i < bytes.len() {
        let c = bytes[i];
        if c == b'=' || c == b'!' || c == b'~' || c == b'>' || c == b'<' || c == b':' || c.is_ascii_whitespace()
        {
            break;
        }
        // also stop before ` contains`
        if input[i..].to_ascii_lowercase().starts_with(" contains") {
            break;
        }
        i += 1;
    }
    if i == 0 {
        return Err("expected column name".into());
    }
    Ok((input[..i].to_string(), i))
}

fn parse_value_token(input: &str) -> Result<(String, usize), String> {
    if input.is_empty() {
        return Ok((String::new(), 0));
    }
    let bytes = input.as_bytes();
    let quote = bytes[0];
    if quote == b'"' || quote == b'\'' || quote == b'`' {
        let mut i = 1;
        let mut out = String::new();
        while i < bytes.len() {
            if bytes[i] == quote {
                return Ok((out, i + 1));
            }
            if bytes[i] == b'\\' && i + 1 < bytes.len() {
                out.push(bytes[i + 1] as char);
                i += 2;
                continue;
            }
            out.push(bytes[i] as char);
            i += 1;
        }
        return Err("unterminated quoted value".into());
    }
    // unquoted value: until whitespace or ` and `
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i].is_ascii_whitespace() {
            break;
        }
        if input[i..].to_ascii_lowercase().starts_with(" and ") {
            break;
        }
        i += 1;
    }
    Ok((input[..i].to_string(), i))
}

fn take_ident(s: &str) -> (String, usize) {
    let mut i = 0;
    let bytes = s.as_bytes();
    while i < bytes.len() {
        let c = bytes[i];
        if c.is_ascii_alphanumeric() || c == b'_' || c == b'-' || c == b'.' {
            i += 1;
        } else {
            break;
        }
    }
    (s[..i].to_string(), i)
}

fn split_first_word(s: &str) -> (&str, &str) {
    let s = s.trim_start();
    if let Some(i) = s.find(char::is_whitespace) {
        (&s[..i], s[i..].trim_start())
    } else {
        (s, "")
    }
}

pub fn unquote_column(token: &str) -> String {
    let b = token.as_bytes();
    if b.len() >= 2 {
        let q = b[0];
        if (q == b'"' || q == b'\'' || q == b'`') && b[b.len() - 1] == q {
            let inner = &token[1..token.len() - 1];
            return inner.replace("\\\"", "\"").replace("\\'", "'").replace("\\`", "`");
        }
    }
    token.to_string()
}

/// Quote a column name for insertion into the command line if needed.
pub fn quote_column_for_input(name: &str) -> String {
    if name.is_empty()
        || name.contains(char::is_whitespace)
        || name.contains('=')
        || name.contains('!')
        || name.contains('~')
        || name.contains('>')
        || name.contains('<')
        || name.contains(':')
        || name.contains('"')
        || name.contains('\'')
    {
        // Prefer double quotes; escape internal ones.
        format!("\"{}\"", name.replace('\"', "\\\""))
    } else {
        name.to_string()
    }
}

// --- Completion ----------------------------------------------------------------

const COMMAND_NAMES: &[&str] = &[
    "filter", "find", "columns", "sort", "goto", "theme", "export", "clear", "help", "quit",
];

#[derive(Debug, Clone, Default)]
pub struct CompletionResult {
    /// Full line to replace the buffer with (cursor at end).
    pub line: String,
    /// Candidates shown in the UI (for the current Tab cycle).
    pub candidates: Vec<String>,
    /// Fully formatted buffer line for each candidate (same order as `candidates`).
    /// Used so Tab can cycle without re-deriving the match prefix from the
    /// already-completed buffer (which would collapse to a single match).
    pub lines: Vec<String>,
    /// Index into candidates that was applied (-1 if none).
    pub index: isize,
}

/// Compute Tab completion for a command-line buffer (no leading `:`).
///
/// `cycle` is the candidate index to apply (`0` = first). Callers that cycle
/// should reuse `lines` / `candidates` from the first result instead of
/// re-invoking this with an already-completed line.
pub fn complete_command_line(line: &str, columns: &[String], cycle: isize) -> CompletionResult {
    // Completing the command word
    if !line.contains(char::is_whitespace) && !looks_like_expression(line) {
        let prefix = line.to_ascii_lowercase();
        let candidates: Vec<String> = COMMAND_NAMES
            .iter()
            .filter(|c| c.starts_with(&prefix))
            .map(|s| (*s).to_string())
            .collect();
        return apply_cycle(line, &candidates, cycle, |c| format!("{c} "));
    }

    let (cmd, rest) = split_first_word(line);
    let cmd_l = cmd.to_ascii_lowercase();

    // `:columns id,name,...` — comma-separated list; complete only the last segment.
    if matches!(cmd_l.as_str(), "columns" | "cols") {
        return complete_columns_list(cmd, rest, columns, cycle, line);
    }

    // `:sort [+-]column`
    if cmd_l == "sort" {
        let (desc_prefix, col_prefix) = if let Some(r) = rest.strip_prefix('-') {
            ("-", r.trim_start())
        } else if let Some(r) = rest.strip_prefix('+') {
            ("+", r.trim_start())
        } else {
            ("", rest.trim_start())
        };
        // If still typing the sign only
        let prefix_lower = unquote_column(col_prefix).to_ascii_lowercase();
        let candidates: Vec<String> = columns
            .iter()
            .filter(|c| c.to_ascii_lowercase().starts_with(&prefix_lower))
            .cloned()
            .collect();
        let sign = desc_prefix.to_string();
        return apply_cycle(line, &candidates, cycle, move |c| {
            format!("sort {sign}{}", quote_column_for_input(c))
        });
    }

    // After `filter ` / `find ` — complete column for the in-progress clause
    if matches!(cmd_l.as_str(), "filter" | "find" | "v" | "search") || looks_like_expression(line)
    {
        let expr_part = if looks_like_expression(line)
            && !matches!(cmd_l.as_str(), "filter" | "find" | "v" | "search")
        {
            line
        } else {
            rest
        };
        // Column token being typed: last clause's column prefix (no operator yet)
        if let Some((prefix_line, col_prefix)) = incomplete_column_prefix(expr_part) {
            let prefix_lower = unquote_column(&col_prefix).to_ascii_lowercase();
            let candidates: Vec<String> = columns
                .iter()
                .filter(|c| c.to_ascii_lowercase().starts_with(&prefix_lower))
                .cloned()
                .collect();
            let head = if looks_like_expression(line)
                && !matches!(cmd_l.as_str(), "filter" | "find" | "v" | "search")
            {
                String::new()
            } else {
                format!("{cmd} ")
            };
            return apply_cycle(line, &candidates, cycle, |c| {
                format!("{head}{prefix_line}{}", quote_column_for_input(c))
            });
        }
    }

    if matches!(cmd_l.as_str(), "theme") {
        let prefix = rest.trim().to_ascii_lowercase();
        let builtins = ["auto", "dark", "light", "grovbox-dark", "nord"];
        let candidates: Vec<String> = builtins
            .iter()
            .filter(|c| c.starts_with(&prefix))
            .map(|s| (*s).to_string())
            .collect();
        return apply_cycle(line, &candidates, cycle, |c| format!("theme {c}"));
    }

    CompletionResult {
        line: line.to_string(),
        candidates: vec![],
        lines: vec![],
        index: -1,
    }
}

/// Complete the last segment of a comma-separated column list for `:columns`.
///
/// Preserves everything before the last comma, e.g. `columns id,na` + Tab
/// becomes `columns id,name` (not `columns name`).
fn complete_columns_list(
    cmd: &str,
    rest: &str,
    columns: &[String],
    cycle: isize,
    original_line: &str,
) -> CompletionResult {
    let (list_prefix, last_token) = split_columns_list_tail(rest);
    let prefix_lower = unquote_column(&last_token).to_ascii_lowercase();

    // Columns already chosen earlier in the list (skip them in suggestions).
    let already: Vec<String> = list_prefix
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(unquote_column)
        .collect();

    let candidates: Vec<String> = columns
        .iter()
        .filter(|c| {
            let cl = c.to_ascii_lowercase();
            cl.starts_with(&prefix_lower)
                && !already
                    .iter()
                    .any(|a| a.eq_ignore_ascii_case(c.as_str()))
        })
        .cloned()
        .collect();

    // If prefix filtered everything out (e.g. exact name already typed), still
    // offer other unused columns when the last token is empty (after a comma).
    let candidates = if candidates.is_empty() && prefix_lower.is_empty() {
        columns
            .iter()
            .filter(|c| {
                !already
                    .iter()
                    .any(|a| a.eq_ignore_ascii_case(c.as_str()))
            })
            .cloned()
            .collect()
    } else {
        candidates
    };

    let head = format!("{cmd} ");
    let list_prefix = list_prefix.to_string();
    apply_cycle(original_line, &candidates, cycle, move |c| {
        let quoted = quote_column_for_input(c);
        if list_prefix.is_empty() {
            format!("{head}{quoted}")
        } else {
            // Keep prior segments exactly as the user typed them (including commas/spaces).
            format!("{head}{list_prefix}{quoted}")
        }
    })
}

/// Split a columns-list argument into `(prefix_including_trailing_comma, last_token)`.
///
/// Respects quotes so commas inside `"Last, Name"` are not separators.
fn split_columns_list_tail(rest: &str) -> (&str, String) {
    let bytes = rest.as_bytes();
    let mut i = 0;
    let mut last_comma: Option<usize> = None;
    let mut quote: Option<u8> = None;

    while i < bytes.len() {
        let b = bytes[i];
        if let Some(q) = quote {
            if b == b'\\' && i + 1 < bytes.len() {
                i += 2;
                continue;
            }
            if b == q {
                quote = None;
            }
            i += 1;
            continue;
        }
        if b == b'"' || b == b'\'' || b == b'`' {
            quote = Some(b);
            i += 1;
            continue;
        }
        if b == b',' {
            last_comma = Some(i);
        }
        i += 1;
    }

    match last_comma {
        Some(pos) => {
            // Include comma and any spaces right after it in the preserved prefix,
            // so we don't invent spacing: keep `id,` or `id, ` as typed.
            let mut prefix_end = pos + 1;
            while prefix_end < bytes.len() && bytes[prefix_end].is_ascii_whitespace() {
                prefix_end += 1;
            }
            let prefix = &rest[..prefix_end];
            let token = rest[prefix_end..].trim_end().to_string();
            (prefix, token)
        }
        None => ("", rest.trim_end().to_string()),
    }
}

fn incomplete_column_prefix(expr: &str) -> Option<(String, String)> {
    // If the expression ends while still reading a column name (no op yet on last token)
    let expr = expr.trim_end();
    if expr.is_empty() {
        return Some((String::new(), String::new()));
    }
    // Walk clauses; if trailing garbage is a partial column, complete it.
    let mut rest = expr;
    let mut prefix_line = String::new();
    loop {
        let trimmed = rest.trim_start();
        let lead_ws = rest.len() - trimmed.len();
        if trimmed.is_empty() {
            return Some((prefix_line, String::new()));
        }
        let lower = trimmed.to_ascii_lowercase();
        if let Some(r) = lower.strip_prefix("and ") {
            let used = trimmed.len() - r.len();
            prefix_line.push_str(&rest[..lead_ws + used]);
            rest = &rest[lead_ws + used..];
            continue;
        }
        match parse_one_clause(trimmed) {
            Ok((_, consumed)) => {
                // fully parsed a clause — continue
                if consumed >= trimmed.len() {
                    // ends exactly on a complete clause; next column is empty
                    prefix_line.push_str(&rest[..lead_ws + consumed]);
                    if !prefix_line.is_empty() && !prefix_line.ends_with(' ') {
                        prefix_line.push(' ');
                    }
                    return Some((prefix_line, String::new()));
                }
                prefix_line.push_str(&rest[..lead_ws + consumed]);
                rest = &rest[lead_ws + consumed..];
                let t = rest.trim_start();
                if t.to_ascii_lowercase().starts_with("and ") {
                    continue;
                }
                if t.is_empty() {
                    if !prefix_line.ends_with(' ') {
                        prefix_line.push(' ');
                    }
                    return Some((prefix_line, String::new()));
                }
                // partial next clause
                continue;
            }
            Err(_) => {
                // partial column at end
                let (col_tok, col_len) = parse_column_token(trimmed).ok()?;
                // ensure no operator fully present after column
                let after = trimmed[col_len..].trim_start();
                if after.is_empty()
                    || (!after.starts_with('=')
                        && !after.starts_with("!=")
                        && !after.starts_with('~')
                        && !after.starts_with('>')
                        && !after.starts_with('<')
                        && !after.starts_with(':')
                        && !after.to_ascii_lowercase().starts_with("contains"))
                {
                    // still typing column (or partial quotes)
                    prefix_line.push_str(&rest[..lead_ws]);
                    return Some((prefix_line, col_tok));
                }
                return None;
            }
        }
    }
}

fn apply_cycle<F>(line: &str, candidates: &[String], cycle: isize, format: F) -> CompletionResult
where
    F: Fn(&str) -> String,
{
    if candidates.is_empty() {
        return CompletionResult {
            line: line.to_string(),
            candidates: vec![],
            lines: vec![],
            index: -1,
        };
    }
    let lines: Vec<String> = candidates.iter().map(|c| format(c)).collect();
    let idx = if cycle < 0 {
        0
    } else {
        (cycle as usize) % candidates.len()
    };
    CompletionResult {
        line: lines[idx].clone(),
        candidates: candidates.to_vec(),
        lines,
        index: idx as isize,
    }
}

impl fmt::Display for ColonCommand {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ColonCommand::Filter(e) => write!(f, "filter {}", e.display()),
            ColonCommand::Find(e) => write!(f, "find {}", e.display()),
            ColonCommand::FindRegex(s) => write!(f, "find {s}"),
            ColonCommand::FilterRegex(s) => write!(f, "filter {s}"),
            ColonCommand::Columns(s) => write!(f, "columns {s}"),
            ColonCommand::Sort { column, descending } => {
                write!(f, "sort {}{column}", if *descending { "-" } else { "+" })
            }
            ColonCommand::Goto(n) => write!(f, "goto {n}"),
            ColonCommand::Theme(t) => write!(f, "theme {t}"),
            ColonCommand::Export(p) => write!(f, "export {p}"),
            ColonCommand::Help => write!(f, "help"),
            ColonCommand::Quit => write!(f, "quit"),
            ColonCommand::ClearSearch => write!(f, "filter"),
            ColonCommand::Clear => write!(f, "clear"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_simple_eq() {
        let e = parse_filter_expr(r#"status=error"#).unwrap();
        assert_eq!(e.clauses.len(), 1);
        assert_eq!(unquote_column(&e.clauses[0].column), "status");
        assert_eq!(e.clauses[0].op, FilterOp::Eq);
        assert_eq!(e.clauses[0].value, "error");
    }

    #[test]
    fn parse_quoted_column_and_and() {
        let e = parse_filter_expr(r#""First Name"~Ann and age>10"#).unwrap();
        assert_eq!(e.clauses.len(), 2);
        assert_eq!(unquote_column(&e.clauses[0].column), "First Name");
        assert_eq!(e.clauses[0].op, FilterOp::Contains);
        assert_eq!(e.clauses[1].op, FilterOp::Gt);
        assert_eq!(e.clauses[1].value, "10");
    }

    #[test]
    fn parse_colon_sugar_and_empty() {
        let e = parse_filter_expr("status:failed email:empty").unwrap();
        assert_eq!(e.clauses[0].op, FilterOp::Eq);
        assert_eq!(e.clauses[0].value, "failed");
        assert_eq!(e.clauses[1].op, FilterOp::Empty);
    }

    #[test]
    fn parse_commands() {
        assert!(matches!(
            parse_colon_command("filter status=error").unwrap(),
            ColonCommand::Filter(_)
        ));
        assert!(matches!(
            parse_colon_command("goto 42").unwrap(),
            ColonCommand::Goto(42)
        ));
        assert!(matches!(
            parse_colon_command("sort -revenue").unwrap(),
            ColonCommand::Sort {
                descending: true,
                ..
            }
        ));
    }

    #[test]
    fn resolve_column_and_match() {
        let headers = vec![
            "status".into(),
            "First Name".into(),
            "age".into(),
        ];
        let e = parse_filter_expr(r#"status=ok "First Name"~Al age>1"#).unwrap();
        let preds = e.resolve(&headers, true).unwrap();
        assert!(preds[0].matches_cell("ok"));
        assert!(!preds[0].matches_cell("fail"));
        assert!(preds[1].matches_cell("Alice"));
        assert!(preds[2].matches_cell("2"));
        assert!(!preds[2].matches_cell("0"));
    }

    #[test]
    fn completion_quotes_spaces() {
        let cols = vec!["id".into(), "First Name".into(), "status".into()];
        let r = complete_command_line("filter Fir", &cols, 0);
        assert!(r.line.contains("First Name") || r.line.contains("\"First Name\""));
        assert!(r.line.starts_with("filter "));
    }

    #[test]
    fn completion_lines_support_cycling() {
        let cols = vec!["alpha".into(), "alpine".into(), "beta".into()];
        let r = complete_command_line("filter al", &cols, 0);
        assert_eq!(r.candidates.len(), 2);
        assert_eq!(r.lines.len(), 2);
        assert_eq!(r.lines[0], "filter alpha");
        assert_eq!(r.lines[1], "filter alpine");
        // Second index via lines (simulates Tab cycle without re-parse)
        assert_eq!(r.lines[1], "filter alpine");
    }

    #[test]
    fn completion_commands_cycle_lines() {
        let r = complete_command_line("f", &[], 0);
        assert!(r.candidates.len() >= 2);
        assert_eq!(r.lines.len(), r.candidates.len());
        assert!(r.lines[0].starts_with("filter") || r.lines.iter().any(|l| l.starts_with("find")));
    }

    #[test]
    fn columns_list_preserves_prior_segments() {
        let cols = vec![
            "id".into(),
            "name".into(),
            "status".into(),
            "First Name".into(),
        ];
        let r = complete_command_line("columns id,na", &cols, 0);
        assert_eq!(r.line, "columns id,name");
        assert!(r.lines.iter().all(|l| l.starts_with("columns id,")));

        // After comma with no prefix — suggest remaining columns, keep `id,`
        let r2 = complete_command_line("columns id,", &cols, 0);
        assert!(r2.line.starts_with("columns id,"));
        assert!(!r2.candidates.iter().any(|c| c == "id"));
        assert!(r2.candidates.iter().any(|c| c == "name" || c == "status"));

        // Quoted multi-word column as second entry
        let r3 = complete_command_line("columns id,Fir", &cols, 0);
        assert_eq!(r3.line, "columns id,\"First Name\"");
    }

    #[test]
    fn split_columns_tail_respects_quotes() {
        let (prefix, tok) = split_columns_list_tail(r#""a,b",sta"#);
        assert_eq!(prefix, r#""a,b","#);
        assert_eq!(tok, "sta");
    }

    #[test]
    fn quote_helper() {
        assert_eq!(quote_column_for_input("status"), "status");
        assert_eq!(quote_column_for_input("First Name"), "\"First Name\"");
    }
}
