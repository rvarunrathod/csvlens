use std::fmt;

#[derive(Clone, PartialEq, Eq, Hash, Copy, Debug)]
pub enum InputMode {
    Default,
    GotoLine,
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
