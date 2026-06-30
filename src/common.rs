use std::fmt;

#[derive(Clone, PartialEq, Eq, Hash, Copy, Debug)]
pub enum InputMode {
    Default,
    GotoLine,
    /// Prompt for column name (`gc` or `:gc` without args).
    GotoColumn,
    /// Waiting for second key after `g` (e.g. `gc` goto column, `gg` top).
    GoPrefix,
    /// Waiting for second key after `z` (column hide/show operations).
    ColumnVisibilityPrefix,
    Find,
    Filter,
    FilterColumns,
    FreezeColumns,
    /// Vim-style colon command line (`:`).
    Command,
    Option,
    Help,
}

impl fmt::Display for InputMode {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}
