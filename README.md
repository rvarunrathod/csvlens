# csvlens

`csvlens` is a command line CSV file viewer. It is like `less` but made
for CSV.

![Demo](.github/demo.gif)

## Features

| Area | What you get |
| --- | --- |
| **Navigation** | Vim-style movement, page/half-page scroll, goto line, freeze columns, line wrap |
| **Search** | Regex find (`/`) and row filter (`&`), next/previous match, column header filter (`*`) |
| **Column-aware filter / find** | Expressions scoped to columns with `=`, `!=`, `~` (contains), `>`, `<`, `:empty` / `:null`; stack clauses with AND |
| **Colon command line (`:`)** | Palette-style commands: `:filter`, `:find`, `:columns`, `:sort`, `:goto`, `:theme`, `:clear`, `:help`, `:q` |
| **fzf-style completion** | Tab in `:` / `&` / `/` opens a floating picker for commands and column names (↑↓ / Tab cycle, Enter accept, Esc dismiss); handles names with spaces via quotes |
| **Selection** | Row / column / cell modes, resize columns, sort (typed + natural), mark rows, copy, echo cell |
| **Themes** | Built-in `auto` / `dark` / `light`, user TOML themes, default via `config.toml` or `CSVLENS_THEME`, themed headers and UI chrome |
| **Help** | Scrollable in-app help card (`H` / `?` / `:help`) with sectioned key reference |
| **Streaming & reload** | Pipe stdin, optional `--auto-reload` when the file changes on disk |
| **Library API** | Embed via `CsvlensOptions` / `run_csvlens_with_options` (optional `Theme`) |

### Column filter expressions

Works in **`:`** commands and, when the text looks like an expression, in **`&`** (filter) and **`/`** (find):

```text
:filter status=error
:filter status!=ok severity>3
:filter "First Name"~Ann and email:empty
:find name~smith
&status:failed age<30          # status:failed is sugar for status=failed
```

| Operator | Meaning | Example |
| --- | --- | --- |
| `=` | Exact match | `status=error` |
| `!=` | Not equal | `status!=ok` |
| `~` or `contains` | Substring | `name~Ann` |
| `>` / `<` | Numeric compare | `age>30` |
| `:empty` / `:null` | Empty or null-like cell | `email:empty` |
| `:value` | Sugar for `=` | `status:failed` |

**Column names with spaces** — quote them so parsing and completion stay unambiguous:

```text
"First Name"=Alice
'Order Date'>2020
`user id`!=0
```

Tab completion inserts the correct quoting automatically (e.g. `First Name` → `"First Name"`). For `:columns`, only the segment **after the last comma** is completed so lists like `id,name,status` build correctly.

### Colon commands

Press **`:`** to open the command line (status shows a leading `:`).

| Command | Description |
| --- | --- |
| `:filter <expr\|regex>` | Keep matching rows (expression or legacy whole-table regex) |
| `:find <expr\|regex>` | Highlight matches |
| `:columns a,b,"Name"` | Show only these columns (comma list or regex) |
| `:sort [+|-]<column>` | Sort ascending (`+`, default) or descending (`-`) |
| `:goto <n>` | Jump to line `n` |
| `:gc <name>` | Jump to column by fuzzy-matched name |
| `:hide` | Toggle hide on the selected (or current) column |
| `:show` | Show all columns |
| `:only` | Show only the selected (or current) column |
| `:theme <name\|path>` | Switch theme (same resolution as `--theme`) |
| `:clear` | Clear find/filter, column filter, and sort |
| `:filter` / `:find` alone | Clear find and row filter only |
| `:help` / `:q` | Help overlay / quit |
| `:export <path>` | Reserved (not implemented yet; use `Ctrl+e` for marked rows) |

### Completion picker

In **`:`**, **`&`**, or **`/`** mode, press **Tab** to open a floating **complete** menu (drawn above the table so grid separators do not clip names):

- **Tab** / **↓** — next candidate (live preview in the command line)
- **Shift+Tab** / **↑** — previous candidate
- **Enter** — accept into the line and close the picker (Enter again runs the command)
- **Esc** — close picker only; second Esc cancels the command line

### Themes & config

See `--theme` under [Optional parameters](#optional-parameters). Defaults live in:

```toml
# ~/.config/csvlens/config.toml
theme = "grovbox-dark"
```

User themes: `~/.config/csvlens/themes/<name>.toml` (override root with `CSVLENS_CONFIG_DIR`).

## Usage

Run `csvlens` by providing the CSV filename:

```
csvlens <filename>
```

Pipe CSV data directly to `csvlens`:

```
<your commands producing some csv data> | csvlens
```
### Key bindings

Key | Action
--- | ---
`hjkl` (or `← ↓ ↑ →`) | Scroll one row or column in the given direction
`Ctrl + f` (or `Page Down`) | Scroll one window down
`Ctrl + b` (or `Page Up`) | Scroll one window up
`Ctrl + d` (or `d`) | Scroll half a window down
`Ctrl + u` (or `u`) | Scroll half a window up
`Ctrl + h` | Scroll one window left
`Ctrl + l` | Scroll one window right
`Ctrl + ←` | Scroll left to first column
`Ctrl + →` | Scroll right to last column
`Ctrl + e` | Print the marked lines to stdout and exit
`G` (or `End`) | Go to bottom
`gg` (or `Home`) | Go to top
`<n>G` | Go to line `n`
`gc` | Go to column by name (fuzzy match; Tab completes)
`/<regex>` | Find content matching regex and highlight matches
`&` / `/` with `col=val` | Column-scoped filter / find (see [Features](#features))
`:` | Open colon command line (see [Features](#features))
`Tab` (in `:` / `&` / `/` / `gc`) | Open fzf-style completion picker
`n` (in Find mode) | Jump to next result
`N` (in Find mode) | Jump to previous result
`&<regex>` | Filter rows using regex (show only matches)
`*<regex>` | Filter columns using regex (show only matches)
`zh` | Toggle hide on the selected (or current) column
`za` (or `zr`) | Show all columns
`zo` | Hide all columns except the selected (or current) one
`TAB` | Toggle between row, column or cell selection modes
`>` | Increase selected column's width
`<` | Decrease selected column's width
`Shift + ↓` (or `J`) | Sort rows or toggle sort direction by the selected column
`Ctrl + j` | Same as above, but sort by natural ordering (e.g. "file2" < "file10")
`#` (in Cell mode) | Find and highlight rows like the selected cell
`@` (in Cell mode) | Filter rows like the selected cell
`y` | Copy the selected row or cell to clipboard
`Enter` (in Cell mode) | Print the selected cell to stdout and exit
`-S` | Toggle line wrapping
`-W` | Toggle line wrapping by words
`f<n>` | Freeze this number of columns from the left
`m` | Mark / unmark the selected row visually
`M` | Clear all row marks
`Ctrl + e` | Print the marked rows (with header) to stdout and exit
`r` | Reset to default view (clear all filters and custom column widths)
`H` (or `?`) | Display help
`q` | Exit

### Optional parameters

* `-d <char>`: Use this delimiter when parsing the CSV
  (e.g. `csvlens file.csv -d '\t'`).

  Specify `-d auto` to auto-detect the delimiter.

* `-t`, `--tab-separated`: Use tab as the delimiter (when specified, `-d` is ignored).

* `-i`, `--ignore-case`: Ignore case when searching. This flag is ignored if any
  uppercase letters are present in the search string.

* `--no-headers`: Do not interpret the first row as headers.

* `--columns <regex>`: Use this regex to select columns to display by default.

  Example: `"column1|column2"` matches `"column1"`, `"column2"`, and also column names like
  `"column11"`, `"column22"`.

* `--filter <regex>`: Use this regex to filter rows to display by default.

  The regex is matched against each cell in every column.

  Example: `"value1|value2"` filters rows with any cells containing `"value1"`, `"value2"`, or text
  like `"my_value1"` or `"value234"`.

* `--find <regex>`: Use this regex to find and highlight matches by default.

  The regex is matched against each cell in every column.

  Example: `"value1|value2"` highlights text in any cells containing `"value1"`, `"value2"`, or
  longer text like `"value1_ok"`.

* `--echo-column <column_name>`: Print the value of this column at the selected
  row to stdout on `Enter` key and then exit.

* `--prompt <prompt>`: Show a custom prompt message in the status bar. Supports ANSI escape codes
  for colored or styled text.

  Example:
  ```bash
  csvlens Pokemon.csv --prompt $'\e[1m\e[32mSelect a Pokémon!\e[0m'
  ```

* `--color-columns` (or `--colorful`): Display each column in a different color.

* `--theme <theme>`: Color theme. Accepts a built-in name, a path to a TOML theme file, or a theme
  name from the config themes directory. You do **not** need to pass this every time — set a default
  instead (see below).

  Built-in themes:
  * `auto` — detect the terminal light/dark mode (fallback when nothing is configured)
  * `dark`
  * `light`

  **Default theme (no flag required)** — priority order:
  1. `--theme` on the command line
  2. `CSVLENS_THEME` environment variable
  3. `theme` in `~/.config/csvlens/config.toml`
  4. `auto`

  ```toml
  # ~/.config/csvlens/config.toml
  theme = "grovbox-dark"
  ```

  ```bash
  export CSVLENS_THEME=grovbox-dark   # optional alternative to config.toml
  csvlens data.csv                   # uses the configured theme
  csvlens data.csv --theme light     # one-off override
  ```

  Theme files are TOML under `~/.config/csvlens/themes/<name>.toml`. All fields are optional and
  fall back to the dark theme when omitted. Colors may be `#hex`, `rgb(r, g, b)`, ANSI names
  (`red`, `lightyellow`, …), or indexed colors (`color42` or `42`).

  ```toml
  # ~/.config/csvlens/themes/nord.toml
  name = "nord"
  header = "#88c0d0"              # column header text color
  row_number = "#4c566a"
  border = "#4c566a"
  selected_foreground = "#eceff4"
  selected_background = "#3b4252"
  marked_foreground = "#eceff4"
  marked_background = "#434c5e"
  found = "#bf616a"
  found_selected_background = "lightyellow"
  status = "#88c0d0"
  column_colors = [
    "#bf616a",
    "#d08770",
    "#ebcb8b",
    "#a3be8c",
    "#b48ead",
  ]
  ```

  The config directory can be overridden with `CSVLENS_CONFIG_DIR` (reads `$CSVLENS_CONFIG_DIR/config.toml`
  and `$CSVLENS_CONFIG_DIR/themes/`). Otherwise `$XDG_CONFIG_HOME/csvlens/` or `~/.config/csvlens/` is used.

## Installation

### Direct download

You can download the `tar.xz` or `zip` file matching your operating system from the
[releases page](https://github.com/YS-L/csvlens/releases), extract it and execute the `csvlens`
binary.

### Homebrew

For macOS, `csvlens` is available on [Homebrew](https://formulae.brew.sh/formula/csvlens). You can
install it using:
```
brew install csvlens
```

### Arch Linux
`csvlens` is available in the [official repositories](https://archlinux.org/packages/extra/x86_64/csvlens). You can install it using:
```
pacman -S csvlens
```

### Windows

For Windows, `csvlens` is available on [winget](https://learn.microsoft.com/en-gb/windows/package-manager/). You can install it using:
```powershell
winget install --id YS-L.csvlens
```

### FreeBSD
`csvlens` is available as a [FreeBSD pkg](https://www.freshports.org/textproc/csvlens/). You can install it using:
```
pkg install csvlens
```

### NetBSD
`csvlens` is available on [pkgsrc](https://ftp.netbsd.org/pub/pkgsrc/current/pkgsrc/textproc/csvlens/index.html). If you're using NetBSD you can install it using:
```
pkgin install csvlens
```

### OpenBSD
`csvlens` is available as an [OpenBSD port](https://cvsweb.openbsd.org/ports/textproc/csvlens/). If you're using OpenBSD 7.6-current or later, you can install it using:
```
doas pkg_add csvlens
```

### Cargo

If you have [Rust](https://www.rust-lang.org/tools/install) installed, `csvlens` is available on
[crates.io](https://crates.io/crates/csvlens) and you can install it using:
```
cargo install csvlens
```

Or, build and install from source after cloning this repo:
```
cargo install --path $(pwd)
```

Requires Rust 1.88.0 or newer.

## Library Usage

This crate allows you to use csvlens as a library.

In your `Cargo.toml`, add the following:

```toml
[dependencies]
csvlens = { version = "0.12.0", default-features = false, features = ["clipboard"] }
```

### Example

Here's a simple example of how to use `csvlens` as a library ([Documentation](https://docs.rs/csvlens/0.12.0/csvlens/index.html)):

```rust
use csvlens::run_csvlens;

let out = run_csvlens(&["/path/to/your.csv"]).unwrap();
if let Some(selected_cell) = out {
    println!("Selected: {}", selected_cell);
}
```

For more advanced usage, you can use `CsvlensOptions` to customize the behavior:

```rust
use csvlens::{run_csvlens_with_options, CsvlensOptions};

let options = CsvlensOptions {
    filename: "/path/to/your.csv".to_string(),
    delimiter: Some("|".to_string()),
    ignore_case: true,
    debug: true,
    ..Default::default()
};
let out = run_csvlens_with_options(options).unwrap();
if let Some(selected_cell) = out {
    println!("Selected: {}", selected_cell);
}
```

See how [qsv](https://github.com/dathere/qsv/tree/master?tab=readme-ov-file#qsv-blazing-fast-data-wrangling-toolkit) uses `csvlens` as a library [here](https://github.com/dathere/qsv/blob/master/src/cmd/lens.rs#L2).
