//! Command line parsing and the `scd --help` user guide.
//!
//! Grammar: `scd [flags] [subcommand | <target>]`. A bare argument is a jump target, which
//! is what makes `scd D:\code` and `scd proj` work without opening the menu.

use std::path::PathBuf;

use crate::color::Palette;

/// What this invocation should do.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    /// No arguments: open the interactive menu.
    Interactive,
    /// `scd <target>`: jump straight away (path, menu number or saved name).
    Jump(String),
    /// `scd list`: print every saved location.
    List,
    /// `scd add [path]`: save a location, defaulting to the current directory.
    Add(Option<String>),
    /// `scd rm <target>`: forget a saved location.
    Remove(String),
    /// `scd mv <target> <position>`: put a saved location at a fixed position (1-based).
    Move { target: String, position: usize },
    /// `scd up <target>` / `scd down <target>`: move one step, clamped at both ends.
    Shift { target: String, delta: i32 },
    /// `scd rename <target> <new name>`: change a location's alias.
    Rename { target: String, name: String },
    /// `scd config`: print which config.json is in use.
    ShowConfig,
    Help,
    Version,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cli {
    pub command: Command,
    /// `--config <file>` override; everything else is resolved in [`crate::config::resolve_path`].
    pub config: Option<PathBuf>,
}

pub fn parse(argv: Vec<String>) -> Result<Cli, String> {
    let mut args = argv.into_iter();
    let _program = args.next();

    let mut config: Option<PathBuf> = None;
    let mut command: Option<Command> = None;
    let mut positional: Vec<String> = Vec::new();
    let mut flags_done = false;

    while let Some(arg) = args.next() {
        if flags_done || arg == "-" || !arg.starts_with('-') {
            positional.push(arg);
            continue;
        }
        let flag_command = match arg.as_str() {
            "--" => {
                flags_done = true;
                continue;
            }
            "-h" | "--help" => Command::Help,
            "-V" | "--version" => Command::Version,
            "-l" | "--list" => Command::List,
            "-c" | "--config" => {
                let value = args
                    .next()
                    .ok_or_else(|| format!("'{arg}' needs a file path, e.g. {arg} D:\\scd\\config.json"))?;
                config = Some(PathBuf::from(crate::paths::unquote(&value)));
                continue;
            }
            "--add" => Command::Add(args.next()),
            "-r" | "--remove" => {
                let value = args
                    .next()
                    .ok_or_else(|| format!("'{arg}' needs a target, e.g. {arg} proj"))?;
                Command::Remove(value)
            }
            other => {
                return Err(format!(
                    "unknown option '{other}' (see 'scd --help'; use 'scd -- <path>' for paths that start with '-')"
                ))
            }
        };
        if command.is_some() {
            return Err(format!("cannot combine '{arg}' with another command"));
        }
        command = Some(flag_command);
    }

    if !positional.is_empty() {
        let keyword = positional[0].to_ascii_lowercase();
        let subcommand = match keyword.as_str() {
            "help" | "?" => Some((Command::Help, 1usize)),
            "version" => Some((Command::Version, 1)),
            "list" | "ls" => Some((Command::List, 1)),
            "config" => Some((Command::ShowConfig, 1)),
            "add" => Some((Command::Add(positional.get(1).cloned()), 2)),
            "rm" | "remove" => match positional.get(1) {
                Some(target) => Some((Command::Remove(target.clone()), 2)),
                None => return Err("'scd rm' needs a number, name or path".to_string()),
            },
            "mv" | "move" => {
                if positional.len() != 3 {
                    return Err(
                        "'scd mv' needs a target and a position, e.g. scd mv proj 1".to_string()
                    );
                }
                let position: usize = match positional[2].parse() {
                    Ok(position) if position >= 1 => position,
                    _ => {
                        return Err(format!(
                            "'{}' is not a position: positions start at 1",
                            positional[2]
                        ))
                    }
                };
                Some((
                    Command::Move { target: positional[1].clone(), position: position - 1 },
                    3,
                ))
            }
            "up" | "down" => match positional.get(1) {
                Some(target) => Some((
                    Command::Shift {
                        target: target.clone(),
                        delta: if keyword == "up" { -1 } else { 1 },
                    },
                    2,
                )),
                None => return Err(format!("'scd {keyword}' needs a number, name or path")),
            },
            "rename" | "name" => {
                if positional.len() < 3 {
                    return Err(
                        "'scd rename' needs a target and a new name, e.g. scd rename 2 proj"
                            .to_string(),
                    );
                }
                // A name may contain spaces, so everything after the target belongs to it.
                let limit = positional.len();
                Some((
                    Command::Rename {
                        target: positional[1].clone(),
                        name: positional[2..].join(" "),
                    },
                    limit,
                ))
            }
            _ => None,
        };

        match subcommand {
            Some((parsed, limit)) => {
                if positional.len() > limit {
                    return Err(format!("unexpected argument '{}'", positional[limit]));
                }
                if command.is_some() {
                    return Err(format!("cannot combine '{}' with another command", positional[0]));
                }
                command = Some(parsed);
            }
            None => {
                if command.is_some() {
                    return Err(format!("unexpected argument '{}'", positional[0]));
                }
                // Rejoin a path the shell split on spaces: `scd D:\my dir` still works.
                command = Some(Command::Jump(positional.join(" ")));
            }
        }
    }

    Ok(Cli { command: command.unwrap_or(Command::Interactive), config })
}

/// The user guide printed by `scd --help` / `scd -h`, coloured when stdout is a terminal.
pub fn help_text() -> String {
    help_with(&Palette::for_stream(crate::ui::Stream::Stdout))
}

/// Build the guide for a specific palette. With colour off this is exactly the plain text.
pub fn help_with(palette: &Palette) -> String {
    colorize_help(&HELP.replace("{version}", env!("CARGO_PKG_VERSION")), palette)
}

/// Line-oriented pretty printer for the guide.
///
/// Roles: section headings bold+underline (readable on light and dark themes), the `scd`
/// command bold green, flags yellow, menu keys yellow with dim brackets, placeholders dim,
/// paths and shipped file names cyan, environment variables yellow, `#` comments dim.
/// Whitespace is copied verbatim so the column alignment survives, and the invariant
/// `strip_ansi(colorize_help(plain)) == plain` is unit tested.
pub fn colorize_help(plain: &str, palette: &Palette) -> String {
    if !palette.enabled {
        return plain.to_string();
    }
    let mut out = String::with_capacity(plain.len() + plain.len() / 2);
    for (index, line) in plain.split('\n').enumerate() {
        if index > 0 {
            out.push('\n');
        }
        out.push_str(&colorize_help_line(line, palette, index == 0));
    }
    out
}

fn colorize_help_line(line: &str, palette: &Palette, title: bool) -> String {
    if line.trim().is_empty() {
        return line.to_string();
    }
    let body = line.trim_end();
    let trailing = &line[body.len()..];
    if title {
        return format!("{}{trailing}", palette.heading(body));
    }

    let text = body.trim_start();
    let indent = &body[..body.len() - text.len()];
    // Section headings: no lowercase letters and a trailing colon ("USAGE:", "中文速览:").
    let heading = text.ends_with(':')
        && text.chars().any(|ch| ch.is_alphabetic())
        && !text.chars().any(|ch| ch.is_lowercase());
    if heading {
        return format!("{indent}{}{trailing}", palette.heading(text));
    }
    format!("{indent}{}{trailing}", colorize_run(text, palette))
}

/// Colour a line token by token while copying every whitespace character unchanged.
fn colorize_run(text: &str, palette: &Palette) -> String {
    let mut out = String::with_capacity(text.len() + 32);
    let mut token = String::new();
    let mut comment = false;
    for ch in text.chars() {
        if ch.is_whitespace() {
            if !token.is_empty() {
                out.push_str(&colorize_token(&token, palette, comment));
                token.clear();
            }
            out.push(ch);
            continue;
        }
        if ch == '#' && token.is_empty() {
            comment = true;
        }
        token.push(ch);
    }
    if !token.is_empty() {
        out.push_str(&colorize_token(&token, palette, comment));
    }
    out
}

fn colorize_token(token: &str, palette: &Palette, comment: bool) -> String {
    if comment {
        return palette.dim(token);
    }
    if token == "scd" || token == "scd.exe" {
        return palette.name(token);
    }
    if token.starts_with('[') && token.ends_with(']') && token.len() > 2 {
        let inner = &token[1..token.len() - 1];
        if !inner.contains('[') && !inner.contains(']') {
            // [c] [a] [q] [1] [12] are keys you press; [path] and friends are placeholders.
            return if inner.chars().count() <= 3 {
                palette.key(inner)
            } else {
                palette.dim(token)
            };
        }
    }
    if token.starts_with('<') && token.ends_with('>') {
        return palette.dim(token);
    }
    if token.starts_with('-') && token.chars().count() > 1 {
        return palette.warn(token);
    }
    if let Some((name, value)) = token.split_once('=') {
        if is_env_name(name) {
            return format!("{}{}", palette.number(name), palette.dim(&format!("={value}")));
        }
    }
    if token.starts_with('$') || is_env_name(token) {
        return palette.number(token);
    }
    if token.len() <= 3
        && token.chars().all(|ch| ch.is_ascii_digit() || ch == '.' || ch == ')')
        && token.chars().any(|ch| ch.is_ascii_digit())
    {
        return palette.number(token);
    }
    if is_pathish(token) {
        return palette.path(token);
    }
    token.to_string()
}

fn is_env_name(token: &str) -> bool {
    token.len() > 2
        && token.contains('_')
        && token.chars().all(|ch| ch.is_ascii_uppercase() || ch.is_ascii_digit() || ch == '_')
}

/// Heuristic: does this token look like a path, or like one of the files we ship?
fn is_pathish(token: &str) -> bool {
    let bare = token
        .trim_matches(|ch: char| matches!(ch, ',' | ';' | ':' | '(' | ')' | '"' | '、' | '（' | '）'));
    if bare.is_empty() {
        return false;
    }
    bare.contains('\\')
        || bare.contains('/')
        || bare.starts_with('~')
        || bare.ends_with(".json")
        || bare.ends_with(".exe")
        || bare.ends_with(".md")
        || bare.ends_with(".ps1")
        || bare.ends_with(".bash")
        || bare.ends_with(".zsh")
        || bare.ends_with(".fish")
        || bare.ends_with(".cmd")
}

const HELP: &str = r#"scd {version} - super cd: jump to your favourite directories from anywhere in the terminal

USAGE:
    scd                      open the interactive menu
    scd <target>             jump straight to a directory, no menu
    scd add [path]           save a new location ("here" or "." = current directory)
    scd rm <target>          forget a saved location
    scd mv <target> <n>      put a saved location at position n (1-based)
    scd up <target>          move a saved location one step up
    scd down <target>        move a saved location one step down
    scd rename <target> <name>
                             change a location's alias (the name may contain spaces)
    scd list                 list every saved location
    scd config               show which config.json is in use
    scd -h, --help           show this guide
    scd -V, --version        show the version

OPTIONS:
    -c, --config <file>      use <file> instead of the auto-detected config.json
    -l, --list               same as "scd list"
    -r, --remove <target>    same as "scd rm <target>"
        --add [path]         same as "scd add [path]"
        --                   treat everything after it as a target (paths starting with '-')

TARGETS are tried in this order:
    1. a directory path      D:\code, ~/code, ..\docs, "C:\my dir", .
    2. a menu number         3          the 3rd location shown by "scd"
    3. a saved name          proj       exact, prefix, substring, path or fuzzy match

MENU KEYS:
    [c]  configure scd       how many locations the menu lists (more settings will follow)
    [a]  add new location    type a path, "here" for the current one, "quit" to cancel
    [1]..[n]  jump           go to that location; the numbers never change by themselves
    [q]  quit                leave without jumping
    Anything else is treated as a target, so you can paste a path at the prompt too.

ORDER AND ALIASES ARE YOURS:
    The list keeps exactly the order you give it - scd never reorders by usage, and new
    locations are appended, so the numbers you already know stay valid. `visits` and
    `last_used_at` in config.json are statistics only; "scd list" shows them.
      rearrange   scd mv <target> <n>  |  scd up <target>  |  scd down <target>
      rename      scd rename <target> <new name>
      by hand     edit config.json - the "locations" array order is the menu order

WHY THE DIRECTORY DOES NOT CHANGE BY ITSELF:
    No program can change the working directory of the shell that started it, so scd
    resolves the directory and hands it back; a tiny shell function does the actual cd.
    Install it once and "scd" behaves exactly like "cd":
      PowerShell   . <install>\shell\scd.ps1        (put that line in $PROFILE)
      bash         source <install>/shell/scd.bash  (put that line in ~/.bashrc)
      zsh          source <install>/shell/scd.zsh   (put that line in ~/.zshrc)
      cmd.exe      put <install>\shell on PATH ahead of <install>\bin (shell\scd.cmd)
    Without the function scd still resolves and prints the target directory, plus a
    ready-to-paste "cd ..." line.

FILES:
    <install>/bin/scd(.exe)  the program - put this directory on PATH
    <install>/config.json    your locations and settings (hand-editable JSON)
    Lookup order: --config, $SCD_CONFIG, $SCD_HOME/config.json,
                  <exe dir>/../config.json when the exe sits in bin/,
                  <exe dir>/config.json, ~/.scd/config.json

ENVIRONMENT:
    SCD_CONFIG=<file>        config file to use
    SCD_HOME=<dir>           install directory holding bin/ and config.json
    SCD_BIN=<exe>            binary the shell function should call
    SCD_TARGET_FILE=<file>   write the chosen directory to <file> (used by the wrappers)
    SCD_CD=1                 the caller is a shell function: suppress the install hint
    SCD_NO_HINT=1            never print the install hint
    SCD_COLOR=auto|always|never
                             colour the output; auto (default) colours only a real terminal
    NO_COLOR=1               same as SCD_COLOR=never (the no-color.org convention)
    CLICOLOR_FORCE=1         colour even when the output is redirected

EXIT STATUS:
    0  ok (also when you quit the menu without jumping)
    1  the target could not be resolved or is not a directory
    2  config.json could not be read, parsed or written
    64 the command line itself was wrong

EXAMPLES:
    scd
    scd D:\data\Projects\scd
    scd 2
    scd proj
    scd add .
    scd add "D:\my dir"
    scd rm proj
    scd rename 2 项目
    scd mv 项目 1
    scd down 3
    scd list
    cd "$(scd proj)"          # portable fallback without the shell function

中文速览:
    scd              打开菜单： [c] 设置 · [a] 添加位置 · [1]..[n] 跳转 · [q] 退出
    scd <目标>       直接跳转，目标可以是路径 / 菜单编号 / 位置名称，不必打开菜单
    scd add .        把当前目录加入配置（追加到末尾，老编号不变）；scd rm <目标> 删除
    scd rename <目标> <新别名>  改别名；scd mv <目标> <位置> / scd up|down <目标> 调顺序
                     顺序完全由你控制：跳转不会重排，也可以直接改 config.json 的数组顺序
    scd config       显示当前使用的 config.json 路径
    配置文件         <安装目录>/config.json（bin/ 的上一级），可用 --config 或 SCD_CONFIG 指定
    切换目录         由 shell/scd.ps1、shell/scd.bash、shell/scd.zsh 中的同名函数完成，
                     安装方法见 README.md；安装后需要把 <安装目录>/bin 加入 PATH 环境变量
    配色             终端里自动上色：别名绿色、路径青色、按键黄色、错误红色、次要信息灰色；
                     SCD_COLOR=always|never 强制开关，遵守 NO_COLOR，重定向时自动关闭
"#;

#[cfg(test)]
mod tests {
    use super::*;

    fn cli(args: &[&str]) -> Result<Cli, String> {
        let mut argv = vec!["scd".to_string()];
        argv.extend(args.iter().map(|arg| (*arg).to_string()));
        parse(argv)
    }

    #[test]
    fn no_arguments_opens_the_menu() {
        assert_eq!(cli(&[]).unwrap(), Cli { command: Command::Interactive, config: None });
    }

    #[test]
    fn bare_argument_is_a_jump_target() {
        assert_eq!(cli(&[r"D:\code"]).unwrap().command, Command::Jump(r"D:\code".to_string()));
        assert_eq!(cli(&["3"]).unwrap().command, Command::Jump("3".to_string()));
        assert_eq!(cli(&["proj"]).unwrap().command, Command::Jump("proj".to_string()));
    }

    #[test]
    fn unquoted_paths_are_rejoined() {
        assert_eq!(
            cli(&[r"D:\my", "dir"]).unwrap().command,
            Command::Jump(r"D:\my dir".to_string())
        );
    }

    #[test]
    fn help_and_version_flags() {
        for arg in ["-h", "--help", "help", "?"] {
            assert_eq!(cli(&[arg]).unwrap().command, Command::Help, "{arg}");
        }
        for arg in ["-V", "--version", "version"] {
            assert_eq!(cli(&[arg]).unwrap().command, Command::Version, "{arg}");
        }
    }

    #[test]
    fn order_and_rename_subcommands() {
        assert_eq!(
            cli(&["mv", "proj", "1"]).unwrap().command,
            Command::Move { target: "proj".to_string(), position: 0 }
        );
        assert_eq!(
            cli(&["move", "3", "7"]).unwrap().command,
            Command::Move { target: "3".to_string(), position: 6 }
        );
        assert_eq!(
            cli(&["up", "proj"]).unwrap().command,
            Command::Shift { target: "proj".to_string(), delta: -1 }
        );
        assert_eq!(
            cli(&["down", "2"]).unwrap().command,
            Command::Shift { target: "2".to_string(), delta: 1 }
        );
        assert_eq!(
            cli(&["rename", "2", "proj"]).unwrap().command,
            Command::Rename { target: "2".to_string(), name: "proj".to_string() }
        );
        assert_eq!(
            cli(&["rename", "proj", "my", "big", "project"]).unwrap().command,
            Command::Rename { target: "proj".to_string(), name: "my big project".to_string() }
        );

        assert!(cli(&["mv", "proj"]).unwrap_err().contains("needs a target and a position"));
        assert!(cli(&["mv", "proj", "1", "2"]).unwrap_err().contains("needs a target and a position"));
        assert!(cli(&["mv", "proj", "x"]).unwrap_err().contains("is not a position"));
        assert!(cli(&["mv", "proj", "0"]).unwrap_err().contains("is not a position"));
        assert!(cli(&["up"]).unwrap_err().contains("'scd up' needs"));
        assert!(cli(&["down"]).unwrap_err().contains("'scd down' needs"));
        assert!(cli(&["rename", "proj"]).unwrap_err().contains("needs a target and a new name"));
        assert!(cli(&["rename"]).unwrap_err().contains("needs a target and a new name"));
    }

    #[test]
    fn subcommands() {
        assert_eq!(cli(&["list"]).unwrap().command, Command::List);
        assert_eq!(cli(&["ls"]).unwrap().command, Command::List);
        assert_eq!(cli(&["-l"]).unwrap().command, Command::List);
        assert_eq!(cli(&["config"]).unwrap().command, Command::ShowConfig);
        assert_eq!(cli(&["add"]).unwrap().command, Command::Add(None));
        assert_eq!(cli(&["--add"]).unwrap().command, Command::Add(None));
        assert_eq!(cli(&["add", "."]).unwrap().command, Command::Add(Some(".".to_string())));
        assert_eq!(cli(&["rm", "proj"]).unwrap().command, Command::Remove("proj".to_string()));
        assert_eq!(cli(&["-r", "2"]).unwrap().command, Command::Remove("2".to_string()));
    }

    #[test]
    fn config_flag_is_captured_separately() {
        let parsed = cli(&["--config", r"D:\scd\config.json", "proj"]).unwrap();
        assert_eq!(parsed.command, Command::Jump("proj".to_string()));
        assert_eq!(parsed.config, Some(PathBuf::from(r"D:\scd\config.json")));

        let parsed = cli(&["-c", "\"D:\\my dir\\config.json\"", "list"]).unwrap();
        assert_eq!(parsed.config, Some(PathBuf::from(r"D:\my dir\config.json")));
        assert_eq!(parsed.command, Command::List);
    }

    #[test]
    fn missing_values_and_unknown_flags_are_errors() {
        assert!(cli(&["--config"]).unwrap_err().contains("needs a file path"));
        assert!(cli(&["-r"]).unwrap_err().contains("needs a target"));
        assert!(cli(&["rm"]).unwrap_err().contains("needs a number, name or path"));
        assert!(cli(&["--nope"]).unwrap_err().contains("unknown option '--nope'"));
        assert!(cli(&["list", "extra"]).unwrap_err().contains("unexpected argument 'extra'"));
        assert!(cli(&["add", "a", "b"]).unwrap_err().contains("unexpected argument 'b'"));
        assert!(cli(&["--list", "extra"]).unwrap_err().contains("unexpected argument"));
        assert!(cli(&["-l", "--list"]).unwrap_err().contains("cannot combine"));
    }

    #[test]
    fn double_dash_protects_paths_that_look_like_flags() {
        assert_eq!(cli(&["--", "-weird-dir"]).unwrap().command, Command::Jump("-weird-dir".to_string()));
    }

    #[test]
    fn a_lone_dash_is_a_target() {
        assert_eq!(cli(&["-"]).unwrap().command, Command::Jump("-".to_string()));
    }

    #[test]
    fn colouring_never_changes_the_guide_text() {
        use crate::color::strip_ansi;
        let plain = help_with(&Palette::off());
        let colored = help_with(&Palette::on());
        assert_eq!(strip_ansi(&colored), plain, "colour must be purely decorative");
        assert!(colored.contains('\x1b'), "forced colour must emit escapes");
        assert!(colored.contains("\x1b[1;4mUSAGE:\x1b[0m"), "section headings are bold+underline");
        assert!(colored.contains("\x1b[1;32mscd\x1b[0m"), "the command name is bold green");
        // Flags use plain yellow, menu keys bold yellow: two different kinds of "type this".
        assert!(colored.contains("\x1b[33m--help\x1b[0m"), "flags are yellow");
        assert!(colored.contains("\x1b[33m--config\x1b[0m"), "flags are yellow");
        assert!(colored.contains("\x1b[1;36mconfig.json\x1b[0m"), "file names are cyan");
        assert!(colored.contains("\x1b[1;33mSCD_COLOR\x1b[0m"), "env vars are yellow");
        assert!(colored.contains("\x1b[2m[\x1b[0m\x1b[1;33mc\x1b[0m"), "menu keys are dim-bracketed yellow");
    }

    #[test]
    fn colouring_preserves_every_line_and_its_alignment() {
        use crate::color::strip_ansi;
        let plain = help_with(&Palette::off());
        let colored = help_with(&Palette::on());
        let plain_lines: Vec<&str> = plain.lines().collect();
        let colored_lines: Vec<String> = colored.lines().map(str::to_string).collect();
        assert_eq!(plain_lines.len(), colored_lines.len());
        for (flat, styled) in plain_lines.iter().zip(colored_lines.iter()) {
            assert_eq!(strip_ansi(styled), *flat, "line differs: {styled:?}");
        }
    }

    #[test]
    fn help_text_covers_the_specification() {
        let help = help_text();
        assert!(help.contains(&format!("scd {}", env!("CARGO_PKG_VERSION"))));
        for expected in [
            "USAGE:",
            "[c]  configure scd",
            "[a]  add new location",
            "[1]..[n]",
            "scd <target>",
            "--help",
            "config.json",
            "SCD_TARGET_FILE",
            "shell/scd.ps1",
            "PATH",
            "中文速览",
            "ORDER AND ALIASES ARE YOURS:",
            "scd mv <target> <n>",
            "scd rename <target> <name>",
            "never reorders by usage",
        ] {
            assert!(help.contains(expected), "help is missing {expected:?}");
        }
        assert!(!help.contains("{version}"), "placeholder must be substituted");
        for expected in ["SCD_COLOR", "NO_COLOR", "配色"] {
            assert!(help.contains(expected), "help is missing {expected:?}");
        }
    }
}