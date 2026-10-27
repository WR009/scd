//! Command line parsing and the `scd --help` user guide.
//!
//! Grammar: `scd [flags] [subcommand | <target>]`. A bare argument is a jump target, which
//! is what makes `scd D:\code` and `scd proj` work without opening the menu.

use std::path::PathBuf;

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

/// The user guide printed by `scd --help` / `scd -h`.
pub fn help_text() -> String {
    HELP.replace("{version}", env!("CARGO_PKG_VERSION"))
}

const HELP: &str = r#"scd {version} - super cd: jump to your favourite directories from anywhere in the terminal

USAGE:
    scd                      open the interactive menu
    scd <target>             jump straight to a directory, no menu
    scd add [path]           save a new location ("here" or "." = current directory)
    scd rm <target>          forget a saved location
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
    [1]..[n]  jump           go to that location; the used one moves to the top next time
    [q]  quit                leave without jumping
    Anything else is treated as a target, so you can paste a path at the prompt too.

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
    scd list
    cd "$(scd proj)"          # portable fallback without the shell function

中文速览:
    scd              打开菜单：[c] 设置、[a] 添加位置、[1]..[n] 跳转、[q] 退出
    scd <目标>       直接跳转，目标可以是路径 / 菜单编号 / 位置名称，不必打开菜单
    scd add .        把当前目录加入配置；scd rm <目标> 删除；scd list 查看全部
    scd config       显示当前使用的 config.json 路径
    配置文件         <安装目录>/config.json（bin/ 的上一级），可用 --config 或 SCD_CONFIG 指定
    切换目录         由 shell/scd.ps1、shell/scd.bash、shell/scd.zsh 中的同名函数完成，
                     安装方法见 README.md；安装后需要把 <安装目录>/bin 加入 PATH 环境变量
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
        ] {
            assert!(help.contains(expected), "help is missing {expected:?}");
        }
        assert!(!help.contains("{version}"), "placeholder must be substituted");
    }
}