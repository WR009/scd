//! End-to-end tests: run the real `scd` binary inside a temporary sandbox and drive it
//! through stdin exactly like a user would.
//!
//! Every test gets its own `SCD_CONFIG`, so the developer's real `~/.scd/config.json` (or
//! the `<install>/config.json` next to the executable) is never touched.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};

use scd::config::Config;
use scd::paths;

const BIN: &str = env!("CARGO_BIN_EXE_scd");
const VERSION: &str = env!("CARGO_PKG_VERSION");

static SEQUENCE: AtomicUsize = AtomicUsize::new(0);

struct Sandbox {
    root: PathBuf,
    config: PathBuf,
    made: usize,
}

impl Sandbox {
    fn new(tag: &str) -> Sandbox {
        let sequence = SEQUENCE.fetch_add(1, Ordering::SeqCst);
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root =
            std::env::temp_dir().join(format!("scd-e2e-{tag}-{}-{sequence}-{nanos}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).expect("create sandbox");
        Sandbox { config: root.join("config.json"), root, made: 0 }
    }

    /// Create a real subdirectory to jump to.
    fn make_dir(&mut self, name: &str) -> PathBuf {
        self.made += 1;
        let path = self.root.join(format!("{}-{name}", self.made));
        fs::create_dir_all(&path).expect("create target directory");
        path
    }

    /// Create a subdirectory and save it through `scd add`.
    fn add_dir(&mut self, name: &str) -> PathBuf {
        let path = self.make_dir(name);
        let out = self.run(&["add", path.to_str().unwrap()]);
        assert_eq!(out.status.code(), Some(0), "scd add failed: {}", stderr(&out));
        path
    }

    fn command(&self) -> Command {
        let mut command = Command::new(BIN);
        command
            .current_dir(&self.root)
            .env("SCD_CONFIG", &self.config)
            .env("SCD_NO_HINT", "1")
            // Hermetic: an ambient CLICOLOR_FORCE in the developer's environment must not
            // change what these tests see. Colour gets its own tests below.
            .env("SCD_COLOR", "never")
            .env_remove("NO_COLOR")
            .env_remove("CLICOLOR")
            .env_remove("CLICOLOR_FORCE")
            .env_remove("TERM")
            .env_remove("SCD_HOME")
            .env_remove("SCD_TARGET_FILE")
            .env_remove("SCD_CD")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        command
    }

    /// Run with arguments and no stdin (immediate EOF).
    fn run(&self, args: &[&str]) -> Output {
        let mut command = self.command();
        command.args(args);
        command.spawn().expect("spawn scd").wait_with_output().expect("wait for scd")
    }

    /// Run the interactive menu and type `input` at it, line by line.
    fn feed(&self, input: &str) -> Output {
        self.feed_args(&[], input)
    }

    fn feed_args(&self, args: &[&str], input: &str) -> Output {
        let mut command = self.command();
        command.args(args);
        let mut child = command.spawn().expect("spawn scd");
        {
            let mut stdin = child.stdin.take().expect("stdin");
            // The child may exit before reading everything (a corrupt config, an unknown
            // command, `quit`), which closes the pipe. That is the behaviour under test,
            // not a harness failure, so a broken pipe is ignored - on a fast static-musl
            // runner the child can win the race, and it did.
            match stdin.write_all(input.as_bytes()) {
                Ok(()) => {}
                Err(err) if err.kind() == std::io::ErrorKind::BrokenPipe => {}
                Err(err) => panic!("write stdin: {err}"),
            }
            let _ = stdin.flush();
        }
        child.wait_with_output().expect("wait for scd")
    }

    /// Same as [`Sandbox::run`] but with colour forced on.
    fn run_colored(&self, args: &[&str]) -> Output {
        let mut command = self.command();
        command.env("SCD_COLOR", "always").args(args);
        command.spawn().expect("spawn scd").wait_with_output().expect("wait for scd")
    }

    /// Same as [`Sandbox::feed_args`] but with colour forced on.
    fn feed_args_colored(&self, args: &[&str]) -> Output {
        let mut command = self.command();
        command.env("SCD_COLOR", "always").args(args);
        let mut child = command.spawn().expect("spawn scd");
        drop(child.stdin.take());
        child.wait_with_output().expect("wait for scd")
    }

    fn config_text(&self) -> String {
        fs::read_to_string(&self.config).unwrap_or_default()
    }

    fn config(&self) -> Config {
        Config::load(&self.config).expect("config must stay readable")
    }
}

impl Drop for Sandbox {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn stdout(out: &Output) -> String {
    String::from_utf8_lossy(&out.stdout).to_string()
}

fn stderr(out: &Output) -> String {
    String::from_utf8_lossy(&out.stderr).to_string()
}

fn combined(out: &Output) -> String {
    format!("{}{}", stdout(out), stderr(out))
}

/// Absolute, normalised form - the spelling `scd` itself reports.
fn abs(path: &Path) -> PathBuf {
    paths::absolute(path)
}

// --------------------------------------------------------------------------- help & version

#[test]
fn help_prints_the_user_guide() {
    let sandbox = Sandbox::new("help");
    for flag in ["--help", "-h", "help"] {
        let out = sandbox.run(&[flag]);
        assert_eq!(out.status.code(), Some(0), "{flag}");
        let text = stdout(&out);
        assert!(text.starts_with(&format!("scd {VERSION} - super cd")), "{text}");
        for expected in [
            "USAGE:",
            "[c]  configure scd",
            "[a]  add new location",
            "[1]..[n]",
            "scd <target>",
            "config.json",
            "shell/scd.ps1",
            "中文速览",
        ] {
            assert!(text.contains(expected), "{flag}: help is missing {expected:?}");
        }
        assert_eq!(stderr(&out), "", "help must not write to stderr");
    }
}

#[test]
fn version_prints_the_crate_version() {
    let sandbox = Sandbox::new("version");
    for flag in ["--version", "-V", "version"] {
        let out = sandbox.run(&[flag]);
        assert_eq!(out.status.code(), Some(0));
        assert_eq!(stdout(&out).trim(), format!("scd {VERSION}"));
    }
}

// --------------------------------------------------------------------------- the menu

#[test]
fn bare_scd_shows_the_documented_menu() {
    let mut sandbox = Sandbox::new("menu");
    let first = sandbox.make_dir("projects");
    let second = sandbox.make_dir("docs");
    sandbox.add_dir("unused");
    // Put the two known directories in front, in a known order.
    sandbox.run(&["add", second.to_str().unwrap()]);
    sandbox.run(&["add", first.to_str().unwrap()]);

    let out = sandbox.feed("q\n");
    assert_eq!(out.status.code(), Some(0), "{}", combined(&out));
    let text = combined(&out);

    // The block from docs/ARCHITECTURE.md, four spaces of indent included.
    assert!(text.contains("    [c] configure scd"), "{text}");
    assert!(text.contains("    [a] add new location"), "{text}");
    assert!(text.contains("    [1] "), "{text}");
    assert!(text.contains("    [2] "), "{text}");
    assert!(text.contains("    [3] "), "{text}");
    assert!(text.contains(&abs(&first).display().to_string()), "{text}");
    assert!(text.contains(&abs(&second).display().to_string()), "{text}");
    assert!(text.contains("[q] quit"), "{text}");
    assert!(text.contains("the order is yours"), "{text}");
    assert!(text.contains("scd> "), "{text}");
    assert!(text.contains("bye"), "{text}");
    assert_eq!(stdout(&out), "", "quitting must not emit a jump target");
}

#[test]
fn empty_config_explains_how_to_start() {
    let sandbox = Sandbox::new("empty");
    let out = sandbox.feed("q\n");
    let text = combined(&out);
    assert!(text.contains("no location saved yet"), "{text}");
    assert!(text.contains("press [a] to add one"), "{text}");
    assert!(text.contains("config:"), "first run shows where the config lives: {text}");
    assert!(!sandbox.config.exists(), "quitting must not create a config file");
}

#[test]
fn menu_hides_locations_beyond_max_display() {
    let mut sandbox = Sandbox::new("maxdisplay");
    for index in 0..4 {
        sandbox.add_dir(&format!("d{index}"));
    }
    let mut cfg = sandbox.config();
    cfg.set_max_display(2).unwrap();
    cfg.save(&sandbox.config).unwrap();

    let text = combined(&sandbox.feed("q\n"));
    assert!(text.contains("[1] "), "{text}");
    assert!(text.contains("[2] "), "{text}");
    assert!(!text.contains("[3] "), "{text}");
    assert!(text.contains("4 saved, 2 hidden"), "{text}");
    assert!(text.contains("max locations to display"), "{text}");
}

#[test]
fn menu_accepts_a_pasted_path_and_quits_on_eof() {
    let mut sandbox = Sandbox::new("paste");
    let target = sandbox.make_dir("pasted");
    let out = sandbox.feed(&format!("{}\n", target.display()));
    assert_eq!(out.status.code(), Some(0), "{}", combined(&out));
    assert_eq!(stdout(&out).trim(), abs(&target).display().to_string());
    assert!(sandbox.config().is_empty(), "pasting a path must not save it");

    // EOF instead of an answer: a helpful note, not a panic.
    let out = sandbox.feed("");
    assert_eq!(out.status.code(), Some(0));
    assert!(combined(&out).contains("no input on stdin"), "{}", combined(&out));
}

#[test]
fn menu_reports_an_unusable_choice_and_keeps_going() {
    let sandbox = Sandbox::new("badchoice");
    let out = sandbox.feed("definitely-not-here\nq\n");
    assert_eq!(out.status.code(), Some(0), "{}", combined(&out));
    let text = combined(&out);
    assert!(text.contains("no saved location matches 'definitely-not-here'"), "{text}");
    assert!(text.contains("[c] configure scd"), "the menu is shown again: {text}");
    assert_eq!(stdout(&out), "");
}

// --------------------------------------------------------------------------- the `a` flow

#[test]
fn add_flow_follows_the_specification() {
    let mut sandbox = Sandbox::new("addflow");
    let target = sandbox.make_dir("spec");
    let out = sandbox.feed(&format!("a\n{}\nq\n", target.display()));
    assert_eq!(out.status.code(), Some(0), "{}", combined(&out));
    let text = combined(&out);

    // Verbatim prompts from docs/ARCHITECTURE.md.
    assert!(
        text.contains("type the new location, or type \"here\" to add the current location"),
        "{text}"
    );
    assert!(text.contains("type \"quit\" to cancel"), "{text}");
    assert!(text.contains("new location: "), "{text}");
    assert!(text.contains("added [1] "), "{text}");
    assert!(text.contains(&abs(&target).display().to_string()), "{text}");

    let cfg = sandbox.config();
    assert_eq!(cfg.locations.len(), 1);
    assert!(cfg.locations[0].name.ends_with("spec"), "{:?}", cfg.locations[0].name);
    assert!(paths::same_path(&cfg.locations[0].path, &abs(&target)));
}

#[test]
fn add_flow_understands_here() {
    let sandbox = Sandbox::new("here");
    let out = sandbox.feed("a\nhere\nq\n");
    assert_eq!(out.status.code(), Some(0), "{}", combined(&out));
    let cfg = sandbox.config();
    assert_eq!(cfg.locations.len(), 1);
    assert!(paths::same_path(&cfg.locations[0].path, &abs(&sandbox.root)));
}

#[test]
fn add_flow_cancels_on_quit_without_touching_the_config() {
    let sandbox = Sandbox::new("addquit");
    let out = sandbox.feed("a\nquit\nq\n");
    assert_eq!(out.status.code(), Some(0), "{}", combined(&out));
    assert!(combined(&out).contains("cancelled"), "{}", combined(&out));
    assert!(!sandbox.config.exists());
}

#[test]
fn add_flow_rejects_a_missing_directory_then_accepts_a_good_one() {
    let mut sandbox = Sandbox::new("addretry");
    let target = sandbox.make_dir("good");
    let out = sandbox.feed(&format!("a\nnope-not-here\n{}\nq\n", target.display()));
    assert_eq!(out.status.code(), Some(0), "{}", combined(&out));
    let text = combined(&out);
    assert!(text.contains("no such directory"), "{text}");
    assert!(text.contains("added [1] "), "{text}");
    assert_eq!(sandbox.config().locations.len(), 1);
}

#[test]
fn add_flow_reports_a_file_and_a_duplicate() {
    let mut sandbox = Sandbox::new("adddupe");
    let target = sandbox.add_dir("dupe");
    let file = sandbox.root.join("notes.txt");
    fs::write(&file, "hello").unwrap();

    let out = sandbox.feed(&format!("a\n{}\nq\n", file.display()));
    let text = combined(&out);
    assert!(text.contains("that is a file, not a directory"), "{text}");

    let out = sandbox.feed(&format!("a\n{}\nq\n", target.display()));
    let text = combined(&out);
    assert!(text.contains("already saved as [1]"), "{text}");
    assert!(text.contains("nothing changed"), "{text}");
    assert_eq!(sandbox.config().locations.len(), 1);
}

#[test]
fn add_flow_handles_quoted_and_unicode_paths() {
    let sandbox = Sandbox::new("unicode");
    let spaced = sandbox.root.join("my projects");
    fs::create_dir_all(&spaced).unwrap();
    let cjk = sandbox.root.join("工作目录");
    fs::create_dir_all(&cjk).unwrap();

    let out = sandbox.feed(&format!("a\n\"{}\"\nq\n", spaced.display()));
    assert_eq!(out.status.code(), Some(0), "{}", combined(&out));
    let out = sandbox.feed(&format!("a\n{}\nq\n", cjk.display()));
    assert_eq!(out.status.code(), Some(0), "{}", combined(&out));

    let cfg = sandbox.config();
    assert_eq!(cfg.locations.len(), 2);
    assert!(paths::same_path(&cfg.locations[0].path, &abs(&spaced)), "added first, stays first");
    assert!(paths::same_path(&cfg.locations[1].path, &abs(&cjk)));
    assert_eq!(cfg.locations[1].name, "工作目录");
    assert!(sandbox.config_text().contains("工作目录"), "config.json keeps UTF-8 names");

    // Jumping back by name works for the unicode entry too.
    let out = sandbox.run(&["工作目录"]);
    assert_eq!(out.status.code(), Some(0), "{}", combined(&out));
    assert_eq!(stdout(&out).trim(), abs(&cjk).display().to_string());
}

#[test]
fn add_non_interactively() {
    let mut sandbox = Sandbox::new("addcli");
    let target = sandbox.make_dir("cli");
    let out = sandbox.run(&["add", target.to_str().unwrap()]);
    assert_eq!(out.status.code(), Some(0), "{}", combined(&out));
    assert!(stdout(&out).starts_with("added [1] "), "{}", stdout(&out));
    assert_eq!(sandbox.config().locations.len(), 1);

    // `scd add` with no argument means "here"; it is appended, so the first entry stays [1].
    let out = sandbox.run(&["add"]);
    assert_eq!(out.status.code(), Some(0), "{}", combined(&out));
    assert!(stdout(&out).starts_with("added [2] "), "{}", stdout(&out));
    assert!(paths::same_path(&sandbox.config().locations[1].path, &abs(&sandbox.root)));
    assert!(paths::same_path(&sandbox.config().locations[0].path, &abs(&target)));

    // A bad path is a target error, not a crash.
    let out = sandbox.run(&["add", "Z:\\definitely\\not\\here"]);
    assert_eq!(out.status.code(), Some(1), "{}", combined(&out));
    assert!(stderr(&out).contains("scd: no such directory"), "{}", stderr(&out));
}

// --------------------------------------------------------------------------- the `c` flow

#[test]
fn configure_flow_sets_max_display() {
    let sandbox = Sandbox::new("configure");
    let out = sandbox.feed("c\n5\nq\n");
    assert_eq!(out.status.code(), Some(0), "{}", combined(&out));
    let text = combined(&out);
    assert!(text.contains("configure scd"), "{text}");
    assert!(text.contains("config file: "), "{text}");
    assert!(text.contains("[1/1] max locations to display"), "{text}");
    assert!(text.contains("current: 9"), "{text}");
    assert!(text.contains("saved "), "{text}");
    assert_eq!(sandbox.config().max_display, 5);
}

#[test]
fn configure_flow_rejects_nonsense_and_keeps_the_old_value() {
    let sandbox = Sandbox::new("configure-bad");
    let out = sandbox.feed("c\nabc\n0\n7\nq\n");
    let text = combined(&out);
    assert!(text.contains("not a whole number"), "{text}");
    assert!(text.contains("out of range"), "{text}");
    assert_eq!(sandbox.config().max_display, 7);
}

#[test]
fn configure_flow_keeps_values_on_bare_enter_and_cancels_on_quit() {
    let sandbox = Sandbox::new("configure-keep");
    let out = sandbox.feed("c\n\nq\n");
    assert!(combined(&out).contains("no changes"), "{}", combined(&out));
    assert_eq!(sandbox.config().max_display, scd::config::DEFAULT_MAX_DISPLAY);
    assert!(!sandbox.config.exists(), "nothing to save means nothing written");

    let out = sandbox.feed("c\nquit\nq\n");
    assert!(combined(&out).contains("cancelled"), "{}", combined(&out));
    assert!(!sandbox.config.exists());
}

#[test]
fn configure_change_takes_effect_on_the_next_menu() {
    let mut sandbox = Sandbox::new("configure-effect");
    for index in 0..3 {
        sandbox.add_dir(&format!("d{index}"));
    }
    let out = sandbox.feed("c\n1\nq\n");
    assert_eq!(out.status.code(), Some(0), "{}", combined(&out));

    let text = combined(&sandbox.feed("q\n"));
    assert!(text.contains("[1] "), "{text}");
    assert!(!text.contains("[2] "), "{text}");
    assert!(text.contains("3 saved, 2 hidden"), "{text}");
}

// --------------------------------------------------------------------------- jumping

#[test]
fn jump_by_path_number_and_name() {
    let mut sandbox = Sandbox::new("jump");
    let first = sandbox.add_dir("alpha");
    let second = sandbox.add_dir("beta");

    // By number: entries are appended, so [1] is the first one added.
    let out = sandbox.run(&["1"]);
    assert_eq!(out.status.code(), Some(0), "{}", combined(&out));
    assert_eq!(stdout(&out).trim(), abs(&first).display().to_string());

    let out = sandbox.run(&["2"]);
    assert_eq!(out.status.code(), Some(0), "{}", combined(&out));
    assert_eq!(stdout(&out).trim(), abs(&second).display().to_string());

    // By name.
    let out = sandbox.run(&["beta"]);
    assert_eq!(out.status.code(), Some(0), "{}", combined(&out));
    assert_eq!(stdout(&out).trim(), abs(&second).display().to_string());

    // By path, for a directory that is not saved at all.
    let unsaved = sandbox.make_dir("unsaved");
    let out = sandbox.run(&[unsaved.to_str().unwrap()]);
    assert_eq!(out.status.code(), Some(0), "{}", combined(&out));
    assert_eq!(stdout(&out).trim(), abs(&unsaved).display().to_string());
}

#[test]
fn stdout_is_only_the_target_directory() {
    let mut sandbox = Sandbox::new("clean-stdout");
    let target = sandbox.add_dir("clean");
    let out = sandbox.run(&[target.to_str().unwrap()]);
    let lines: Vec<String> = stdout(&out).lines().map(str::trim).filter(|line| !line.is_empty()).map(str::to_string).collect();
    assert_eq!(lines, vec![abs(&target).display().to_string()], "cd \"$(scd ...)\" must stay safe");
    assert!(stderr(&out).contains("-> "), "the human-readable line goes to stderr");
}

#[test]
fn target_file_contract_used_by_the_shell_wrappers() {
    let mut sandbox = Sandbox::new("targetfile");
    let target = sandbox.add_dir("wrapped");
    let target_file = sandbox.root.join("scd-target.txt");

    let mut command = sandbox.command();
    command
        .env("SCD_TARGET_FILE", &target_file)
        .env_remove("SCD_NO_HINT")
        .arg(target.to_str().unwrap());
    let out = command.output().expect("run scd");

    assert_eq!(out.status.code(), Some(0), "{}", combined(&out));
    let written = fs::read_to_string(&target_file).expect("target file");
    assert_eq!(written.trim(), abs(&target).display().to_string());
    assert!(!stdout(&out).lines().any(|line| line.trim() == abs(&target).display().to_string()));
    assert!(stdout(&out).contains("-> "), "the UI stays on stdout: {}", stdout(&out));
    assert!(
        !combined(&out).contains("cannot change this shell's directory"),
        "a wrapped call must not nag: {}",
        combined(&out)
    );
}

#[test]
fn unwrapped_call_explains_how_to_change_directory() {
    let mut sandbox = Sandbox::new("hint");
    let target = sandbox.add_dir("hinted");
    let mut command = sandbox.command();
    command.env_remove("SCD_NO_HINT").arg(target.to_str().unwrap());
    let out = command.output().expect("run scd");
    let text = combined(&out);
    assert!(text.contains("cannot change this shell's directory by itself"), "{text}");
    assert!(text.contains(&format!("cd \"{}\"", abs(&target).display())), "{text}");

    // SCD_CD=1 (the portable `cd "$(scd)"` wrapper) silences the same hint.
    let mut command = sandbox.command();
    command.env_remove("SCD_NO_HINT").env("SCD_CD", "1").arg(target.to_str().unwrap());
    let out = command.output().expect("run scd");
    assert!(!combined(&out).contains("cannot change"), "{}", combined(&out));
    assert_eq!(stdout(&out).trim(), abs(&target).display().to_string());
}

#[test]
fn jumping_never_reorders_but_does_count_visits() {
    let mut sandbox = Sandbox::new("stable-order");
    let alpha = sandbox.add_dir("alpha");
    let beta = sandbox.add_dir("beta");
    assert!(paths::same_path(&sandbox.config().locations[0].path, &abs(&alpha)));

    let out = sandbox.feed("2\n"); // 2 == beta
    assert_eq!(out.status.code(), Some(0), "{}", combined(&out));
    assert_eq!(stdout(&out).trim(), abs(&beta).display().to_string());

    let cfg = sandbox.config();
    assert!(
        paths::same_path(&cfg.locations[0].path, &abs(&alpha)),
        "the first entry stays first after a jump"
    );
    assert!(paths::same_path(&cfg.locations[1].path, &abs(&beta)));
    assert_eq!(cfg.locations[0].visits, 0);
    assert_eq!(cfg.locations[1].visits, 1, "usage is recorded as metadata only");
    assert!(cfg.locations[1].last_used_at > 0);

    // The next menu shows exactly the same numbers.
    let text = combined(&sandbox.feed("q\n"));
    let alpha_line = text
        .lines()
        .find(|line| line.contains(&abs(&alpha).display().to_string()))
        .expect("alpha is listed");
    assert!(alpha_line.contains("[1]"), "{alpha_line}");
    assert!(text.contains("the order is yours"), "the menu advertises the reorder commands: {text}");
}

#[test]
fn unknown_targets_fail_with_a_usable_message() {
    let mut sandbox = Sandbox::new("unknown");
    sandbox.add_dir("only");

    let out = sandbox.run(&["definitely-not-here"]);
    assert_eq!(out.status.code(), Some(1), "{}", combined(&out));
    assert_eq!(stdout(&out), "", "a failed jump must not print a target");
    assert!(
        stderr(&out).contains("scd: no saved location matches 'definitely-not-here'"),
        "{}",
        stderr(&out)
    );
    assert!(stderr(&out).contains("hint:"), "{}", stderr(&out));

    let out = sandbox.run(&["9"]);
    assert_eq!(out.status.code(), Some(1));
    assert!(stderr(&out).contains("out of range"), "{}", stderr(&out));
    assert!(stderr(&out).contains("1..1"), "{}", stderr(&out));

    let file = sandbox.root.join("file.txt");
    fs::write(&file, "x").unwrap();
    let out = sandbox.run(&[file.to_str().unwrap()]);
    assert_eq!(out.status.code(), Some(1));
    assert!(stderr(&out).contains("file, not a directory"), "{}", stderr(&out));
}

#[test]
fn relative_paths_dot_segments_and_tilde_are_resolved() {
    let sandbox = Sandbox::new("relative");
    let nested = sandbox.root.join("nested");
    fs::create_dir_all(nested.join("deep")).unwrap();

    let out = sandbox.run(&["nested/deep"]);
    assert_eq!(out.status.code(), Some(0), "{}", combined(&out));
    assert_eq!(stdout(&out).trim(), abs(&nested.join("deep")).display().to_string());

    let out = sandbox.run(&["."]);
    assert_eq!(out.status.code(), Some(0), "{}", combined(&out));
    assert_eq!(stdout(&out).trim(), abs(&sandbox.root).display().to_string());

    let out = sandbox.run(&["nested/../nested"]);
    assert_eq!(out.status.code(), Some(0), "{}", combined(&out));
    assert_eq!(stdout(&out).trim(), abs(&nested).display().to_string());

    let out = sandbox.run(&["~"]);
    assert_eq!(out.status.code(), Some(0), "{}", combined(&out));
    let home = stdout(&out).trim().to_string();
    assert!(Path::new(&home).is_dir(), "~ must resolve to a directory: {home}");
    assert_eq!(home, paths::expand("~").display().to_string());
}

#[test]
fn double_dash_protects_a_path_that_starts_with_a_dash() {
    let sandbox = Sandbox::new("dash");
    let odd = sandbox.root.join("-weird-");
    fs::create_dir_all(&odd).unwrap();
    let out = sandbox.run(&["--", "-weird-"]);
    assert_eq!(out.status.code(), Some(0), "{}", combined(&out));
    assert_eq!(stdout(&out).trim(), abs(&odd).display().to_string());
}

// --------------------------------------------------------------------------- list / rm / config

#[test]
fn list_shows_every_location_including_hidden_ones() {
    let mut sandbox = Sandbox::new("list");
    let first = sandbox.add_dir("one");
    sandbox.add_dir("two");
    sandbox.add_dir("three");
    let mut cfg = sandbox.config();
    cfg.set_max_display(2).unwrap();
    cfg.save(&sandbox.config).unwrap();

    let out = sandbox.run(&["list"]);
    assert_eq!(out.status.code(), Some(0), "{}", combined(&out));
    let text = stdout(&out);
    assert!(text.contains(&format!("# config: {}", sandbox.config.display())), "{text}");
    assert!(text.contains("3 location(s) saved"), "{text}");
    assert!(text.contains("max locations to display: 2"), "{text}");
    assert!(text.contains("[ 1]") && text.contains("menu"), "{text}");
    assert!(text.contains("[ 3]") && text.contains("hidden"), "{text}");
    assert!(text.contains(&abs(&first).display().to_string()), "{text}");
    assert_eq!(stderr(&out), "");
}

#[test]
fn remove_forgets_a_location() {
    let mut sandbox = Sandbox::new("rm");
    let alpha = sandbox.add_dir("alpha");
    sandbox.add_dir("beta");

    let out = sandbox.run(&["rm", "beta"]);
    assert_eq!(out.status.code(), Some(0), "{}", combined(&out));
    assert!(stdout(&out).starts_with("removed [2] "), "{}", stdout(&out));
    let cfg = sandbox.config();
    assert_eq!(cfg.locations.len(), 1);
    assert!(paths::same_path(&cfg.locations[0].path, &abs(&alpha)));

    // By number and by path as well.
    let out = sandbox.run(&["rm", "1"]);
    assert_eq!(out.status.code(), Some(0), "{}", combined(&out));
    assert!(sandbox.config().is_empty());

    let out = sandbox.run(&["rm", "ghost"]);
    assert_eq!(out.status.code(), Some(1));
    assert!(stderr(&out).contains("scd:"), "{}", stderr(&out));
}

#[test]
fn config_command_prints_the_file_in_use() {
    let sandbox = Sandbox::new("configcmd");
    let out = sandbox.run(&["config"]);
    assert_eq!(out.status.code(), Some(0), "{}", combined(&out));
    assert_eq!(stdout(&out).trim(), sandbox.config.display().to_string());
}

#[test]
fn config_flag_uses_another_file() {
    let mut sandbox = Sandbox::new("configflag");
    // Deliberately not add_dir(): the default config.json must stay untouched here.
    let target = sandbox.make_dir("flagged");
    let other = sandbox.root.join("other.json");

    let out =
        sandbox.run(&["--config", other.to_str().unwrap(), "add", target.to_str().unwrap()]);
    assert_eq!(out.status.code(), Some(0), "{}", combined(&out));
    assert!(other.exists(), "the override file is used");
    assert!(!sandbox.config.exists(), "the default file stays untouched");

    let out = sandbox.run(&["--config", other.to_str().unwrap(), "1"]);
    assert_eq!(out.status.code(), Some(0), "{}", combined(&out));
    assert_eq!(stdout(&out).trim(), abs(&target).display().to_string());

    let out = sandbox.run(&["--config"]);
    assert_eq!(out.status.code(), Some(64), "{}", combined(&out));
    assert!(stderr(&out).contains("needs a file path"), "{}", stderr(&out));
}

// --------------------------------------------------------------------------- config file handling

#[test]
fn corrupt_config_is_reported_and_never_overwritten() {
    let mut sandbox = Sandbox::new("corrupt");
    fs::write(&sandbox.config, "{ \"max_display\": oops }").unwrap();

    let out = sandbox.feed("q\n");
    assert_eq!(out.status.code(), Some(2), "{}", combined(&out));
    assert!(stderr(&out).contains("cannot parse"), "{}", stderr(&out));
    assert!(stderr(&out).contains(&sandbox.config.display().to_string()), "{}", stderr(&out));
    assert_eq!(sandbox.config_text(), "{ \"max_display\": oops }");

    // Even a write command refuses to clobber it.
    let target = sandbox.make_dir("safe");
    let out = sandbox.run(&["add", target.to_str().unwrap()]);
    assert_eq!(out.status.code(), Some(2), "{}", combined(&out));
    assert_eq!(sandbox.config_text(), "{ \"max_display\": oops }");
}

#[test]
fn hand_edited_config_is_accepted() {
    let mut sandbox = Sandbox::new("handmade");
    let target = sandbox.make_dir("hand");
    let escaped = target.display().to_string().replace('\\', "\\\\");
    fs::write(
        &sandbox.config,
        format!("{{\n  \"max_display\": 3,\n  \"locations\": [{{\"path\": \"{escaped}\", \"name\": \"手写\"}}]\n}}\n"),
    )
    .unwrap();

    let out = sandbox.run(&["手写"]);
    assert_eq!(out.status.code(), Some(0), "{}", combined(&out));
    assert_eq!(stdout(&out).trim(), abs(&target).display().to_string());
    let cfg = sandbox.config();
    assert_eq!(cfg.max_display, 3);
    assert_eq!(cfg.locations[0].name, "手写");
    assert_eq!(cfg.locations[0].visits, 1, "the jump was recorded");
}

#[test]
fn empty_config_file_starts_fresh() {
    let sandbox = Sandbox::new("blankconfig");
    fs::write(&sandbox.config, "").unwrap();
    let out = sandbox.feed("q\n");
    assert_eq!(out.status.code(), Some(0), "{}", combined(&out));
    assert!(combined(&out).contains("no location saved yet"), "{}", combined(&out));
}

#[test]
fn config_directory_is_created_on_first_save() {
    let sandbox = Sandbox::new("nestedconfig");
    let nested = sandbox.root.join("a").join("b").join("config.json");
    let target = sandbox.root.join("target");
    fs::create_dir_all(&target).unwrap();

    let out = sandbox.run(&["--config", nested.to_str().unwrap(), "add", target.to_str().unwrap()]);
    assert_eq!(out.status.code(), Some(0), "{}", combined(&out));
    assert!(nested.exists(), "parents are created");
    let text = fs::read_to_string(&nested).unwrap();
    assert!(text.contains("\"version\": 1"), "{text}");
    assert!(text.contains("\"max_display\": 9"), "{text}");
    assert!(!nested.with_file_name("config.json.tmp").exists(), "no temp file left behind");
}

// --------------------------------------------------------------------------- usage errors

#[test]
fn usage_errors_exit_64() {
    let sandbox = Sandbox::new("usage");
    for args in [vec!["--nope"], vec!["rm"], vec!["list", "extra"], vec!["-l", "--list"]] {
        let refs: Vec<&str> = args.iter().map(|arg| *arg).collect();
        let out = sandbox.run(&refs);
        assert_eq!(out.status.code(), Some(64), "{args:?}: {}", combined(&out));
        assert!(stderr(&out).contains("scd: "), "{args:?}: {}", stderr(&out));
        assert!(stderr(&out).contains("scd --help"), "{args:?}: {}", stderr(&out));
        assert_eq!(stdout(&out), "");
    }
}

#[test]
fn ambiguous_name_lists_the_choices() {
    let sandbox = Sandbox::new("ambiguous");
    let alpha = sandbox.root.join("proj-alpha");
    let beta = sandbox.root.join("proj-beta");
    fs::create_dir_all(&alpha).unwrap();
    fs::create_dir_all(&beta).unwrap();
    sandbox.run(&["add", alpha.to_str().unwrap()]);
    sandbox.run(&["add", beta.to_str().unwrap()]);

    let out = sandbox.run(&["proj"]);
    assert_eq!(out.status.code(), Some(1), "{}", combined(&out));
    let text = stderr(&out);
    assert!(text.contains("matches 2 saved locations"), "{text}");
    assert!(text.contains("proj-alpha") && text.contains("proj-beta"), "{text}");
    assert!(text.contains("[1] [2]"), "{text}");

    // Numbers disambiguate; [1] is the one added first.
    let out = sandbox.run(&["1"]);
    assert_eq!(out.status.code(), Some(0), "{}", combined(&out));
    assert_eq!(stdout(&out).trim(), abs(&alpha).display().to_string());
}

#[test]
fn read_only_commands_never_create_a_config_file() {
    let sandbox = Sandbox::new("readonly");
    for args in [vec!["list"], vec!["config"], vec!["--help"], vec!["q"]] {
        let refs: Vec<&str> = args.iter().map(|arg| *arg).collect();
        sandbox.run(&refs);
    }
    assert!(!sandbox.config.exists(), "read-only commands must not write a config file");
}
// --------------------------------------------------------------------------- colour

/// Strip ANSI escapes so coloured output can be compared with its plain counterpart.
fn plain(text: &str) -> String {
    scd::color::strip_ansi(text)
}

fn has_escapes(text: &str) -> bool {
    text.contains('\u{1b}')
}

#[test]
fn colour_is_off_when_output_is_redirected() {
    let mut sandbox = Sandbox::new("color-auto");
    let target = sandbox.add_dir("plain");
    // The sandbox pins SCD_COLOR=never; "auto" would behave the same because stdout is a pipe.
    let mut command = sandbox.command();
    command.env_remove("SCD_COLOR").arg(target.to_str().unwrap());
    let out = command.output().expect("run scd");
    assert!(!has_escapes(&stdout(&out)), "redirected stdout must stay byte clean");
    assert!(!has_escapes(&stderr(&out)), "redirected stderr must stay byte clean");
}

#[test]
fn scd_color_always_paints_the_menu_but_not_the_target() {
    let mut sandbox = Sandbox::new("color-always");
    let target = sandbox.add_dir("painted");

    // Menu chrome: coloured.
    let out = sandbox.feed_args_colored(&["list"]);
    let text = stdout(&out);
    assert!(has_escapes(&text), "SCD_COLOR=always must emit escapes: {text:?}");
    assert!(text.contains("\u{1b}[1;32m"), "alias in bold green: {text:?}");
    assert!(text.contains("\u{1b}[1;36m"), "path in bold cyan: {text:?}");
    assert_eq!(plain(&text), stdout(&sandbox.run(&["list"])), "colour changes nothing but style");

    // The machine-readable jump target: never coloured, whatever the user asks for.
    let out = sandbox.run_colored(&[target.to_str().unwrap()]);
    assert_eq!(out.status.code(), Some(0), "{}", combined(&out));
    let raw = stdout(&out);
    assert!(!has_escapes(&raw), "stdout is consumed by `cd \"$(scd ...)\"`: {raw:?}");
    assert_eq!(raw.trim(), abs(&target).display().to_string());
    assert!(has_escapes(&stderr(&out)), "the human line on stderr is coloured");
    assert!(stderr(&out).contains("\u{1b}[1;36m"), "{:?}", stderr(&out));
}

#[test]
fn target_file_payload_is_never_coloured() {
    let mut sandbox = Sandbox::new("color-targetfile");
    let target = sandbox.add_dir("wrapped-color");
    let target_file = sandbox.root.join("scd-target.txt");

    let mut command = sandbox.command();
    command
        .env("SCD_COLOR", "always")
        .env("SCD_TARGET_FILE", &target_file)
        .arg(target.to_str().unwrap());
    let out = command.output().expect("run scd");
    assert_eq!(out.status.code(), Some(0), "{}", combined(&out));

    let written = fs::read_to_string(&target_file).expect("target file");
    assert!(!has_escapes(&written), "the shell wrapper reads this verbatim: {written:?}");
    assert_eq!(written.trim(), abs(&target).display().to_string());
    assert!(has_escapes(&stdout(&out)), "the UI on stdout is coloured");
}

#[test]
fn no_color_beats_clicolor_force_and_scd_color_auto() {
    let mut sandbox = Sandbox::new("no-color");
    sandbox.add_dir("quiet");
    let mut command = sandbox.command();
    command
        .env_remove("SCD_COLOR")
        .env("NO_COLOR", "1")
        .env("CLICOLOR_FORCE", "1")
        .arg("list");
    let out = command.output().expect("run scd");
    assert!(!has_escapes(&stdout(&out)), "NO_COLOR must win: {:?}", stdout(&out));
}

#[test]
fn scd_color_never_keeps_config_writes_plain() {
    let sandbox = Sandbox::new("color-config-json");
    let mut command = sandbox.command();
    command.env("SCD_COLOR", "always").args(["add", "."]);
    let out = command.output().expect("run scd");
    assert_eq!(out.status.code(), Some(0), "{}", combined(&out));
    let text = sandbox.config_text();
    assert!(!has_escapes(&text), "config.json must never contain escape codes: {text}");
    assert!(serde_like_parse(&text), "config.json stays valid JSON");
    assert!(has_escapes(&stdout(&out)), "the confirmation message is coloured");
    assert_eq!(plain(&stdout(&out)).trim_start(), plain(&stdout(&out)).trim_start());
}

/// The project has no JSON dependency in tests either, so validate by round-tripping
/// through the library that wrote the file.
fn serde_like_parse(text: &str) -> bool {
    scd::json::Json::parse(text).is_ok()
}

#[test]
fn help_is_coloured_only_when_asked() {
    let sandbox = Sandbox::new("color-help");
    let mut command = sandbox.command();
    command.env("SCD_COLOR", "always").arg("--help");
    let colored = stdout(&command.output().expect("run scd"));
    assert!(has_escapes(&colored));
    assert!(colored.contains("\u{1b}[1;4mUSAGE:\u{1b}[0m"), "headings are bold+underline");

    let plain_help = stdout(&sandbox.run(&["--help"]));
    assert!(!has_escapes(&plain_help));
    assert_eq!(plain(&colored), plain_help, "the guide text is identical in both modes");
}

#[test]
fn errors_and_hints_are_coloured() {
    let mut sandbox = Sandbox::new("color-errors");
    sandbox.add_dir("only-color");
    let mut command = sandbox.command();
    command.env("SCD_COLOR", "always").arg("not-a-real-place");
    let out = command.output().expect("run scd");
    assert_eq!(out.status.code(), Some(1));
    let err = stderr(&out);
    assert!(err.contains("\u{1b}[1;31mscd:\u{1b}[0m"), "red `scd:` prefix: {err:?}");
    assert!(err.contains("\u{1b}[33mhint:\u{1b}[0m"), "yellow hint: {err:?}");
    assert_eq!(plain(&err), stderr(&sandbox.run(&["not-a-real-place"])));
    assert_eq!(stdout(&out), "", "a failed jump still prints no target");
}
// --------------------------------------------------------------------------- order & aliases

/// The saved aliases, in menu order.
fn order(sandbox: &Sandbox) -> Vec<String> {
    sandbox.config().locations.iter().map(|location| location.name.clone()).collect()
}

#[test]
fn add_appends_so_existing_numbers_stay_valid() {
    let mut sandbox = Sandbox::new("append");
    let alpha = sandbox.add_dir("alpha");
    let beta = sandbox.add_dir("beta");
    assert_eq!(stdout(&sandbox.run(&["1"])).trim(), abs(&alpha).display().to_string());
    assert_eq!(stdout(&sandbox.run(&["2"])).trim(), abs(&beta).display().to_string());

    let gamma = sandbox.make_dir("gamma");
    let out = sandbox.run(&["add", gamma.to_str().unwrap()]);
    assert!(stdout(&out).starts_with("added [3] "), "{}", stdout(&out));

    let cfg = sandbox.config();
    assert_eq!(cfg.locations.len(), 3);
    assert!(paths::same_path(&cfg.locations[0].path, &abs(&alpha)));
    assert!(paths::same_path(&cfg.locations[1].path, &abs(&beta)));
    assert!(paths::same_path(&cfg.locations[2].path, &abs(&gamma)));

    // The numbers the user already knows still point at the same places.
    assert_eq!(stdout(&sandbox.run(&["1"])).trim(), abs(&alpha).display().to_string());
    assert_eq!(stdout(&sandbox.run(&["2"])).trim(), abs(&beta).display().to_string());
    assert_eq!(stdout(&sandbox.run(&["3"])).trim(), abs(&gamma).display().to_string());
}

#[test]
fn mv_up_and_down_reorder_and_persist() {
    let mut sandbox = Sandbox::new("reorder");
    sandbox.add_dir("one");
    sandbox.add_dir("two");
    sandbox.add_dir("three");
    for (index, name) in [(1, "one"), (2, "two"), (3, "three")] {
        let out = sandbox.run(&["rename", &index.to_string(), name]);
        assert_eq!(out.status.code(), Some(0), "{}", combined(&out));
    }
    assert_eq!(order(&sandbox), vec!["one", "two", "three"]);

    let out = sandbox.run(&["mv", "three", "1"]);
    assert_eq!(out.status.code(), Some(0), "{}", combined(&out));
    assert!(stdout(&out).contains("moved three: [3] -> [1]"), "{}", stdout(&out));
    assert!(stdout(&out).contains("[1] three"), "the new order is printed: {}", stdout(&out));
    assert_eq!(order(&sandbox), vec!["three", "one", "two"]);

    let out = sandbox.run(&["down", "three"]);
    assert!(stdout(&out).contains("moved three: [1] -> [2]"), "{}", stdout(&out));
    assert_eq!(order(&sandbox), vec!["one", "three", "two"]);

    let out = sandbox.run(&["up", "2"]);
    assert_eq!(out.status.code(), Some(0), "{}", combined(&out));
    assert_eq!(order(&sandbox), vec!["three", "one", "two"]);

    // Both ends are clamped, not wrapped.
    let out = sandbox.run(&["up", "1"]);
    assert_eq!(out.status.code(), Some(0), "{}", combined(&out));
    assert!(stdout(&out).contains("already at the top"), "{}", stdout(&out));
    let out = sandbox.run(&["down", "3"]);
    assert!(stdout(&out).contains("already at the bottom"), "{}", stdout(&out));
    assert_eq!(order(&sandbox), vec!["three", "one", "two"]);

    // Out of range, and a target that is not saved: nothing changes.
    let out = sandbox.run(&["mv", "one", "9"]);
    assert_eq!(out.status.code(), Some(1), "{}", combined(&out));
    assert!(stderr(&out).contains("1..3"), "{}", stderr(&out));
    let out = sandbox.run(&["mv", "ghost", "1"]);
    assert_eq!(out.status.code(), Some(1), "{}", combined(&out));
    assert!(stderr(&out).contains("scd:"), "{}", stderr(&out));
    assert_eq!(order(&sandbox), vec!["three", "one", "two"]);
    assert_eq!(stdout(&sandbox.run(&["1"])).trim(), order_path(&sandbox, 0));
}

fn order_path(sandbox: &Sandbox, index: usize) -> String {
    sandbox.config().locations[index].path.display().to_string()
}

#[test]
fn rename_changes_the_alias_and_keeps_the_path() {
    let mut sandbox = Sandbox::new("rename");
    let target = sandbox.add_dir("before");

    let out = sandbox.run(&["rename", "1", "之后"]);
    assert_eq!(out.status.code(), Some(0), "{}", combined(&out));
    let text = stdout(&out);
    assert!(text.contains("renamed [1]"), "{text}");
    assert!(text.contains("之后"), "{text}");
    assert!(text.contains(&abs(&target).display().to_string()), "{text}");

    let cfg = sandbox.config();
    assert_eq!(cfg.locations[0].name, "之后");
    assert!(paths::same_path(&cfg.locations[0].path, &abs(&target)), "the path is untouched");
    assert_eq!(stdout(&sandbox.run(&["之后"])).trim(), abs(&target).display().to_string());

    // Names may contain spaces.
    let out = sandbox.run(&["rename", "1", "my", "big", "project"]);
    assert_eq!(out.status.code(), Some(0), "{}", combined(&out));
    assert_eq!(sandbox.config().locations[0].name, "my big project");
    assert_eq!(
        stdout(&sandbox.run(&["my big project"])).trim(),
        abs(&target).display().to_string()
    );
}

#[test]
fn rename_refuses_empty_and_duplicate_names() {
    let mut sandbox = Sandbox::new("rename-bad");
    sandbox.add_dir("first");
    sandbox.add_dir("second");
    assert_eq!(sandbox.run(&["rename", "1", "alpha"]).status.code(), Some(0));

    let out = sandbox.run(&["rename", "2", "alpha"]);
    assert_eq!(out.status.code(), Some(1), "{}", combined(&out));
    assert!(stderr(&out).contains("already used by location [1]"), "{}", stderr(&out));

    let out = sandbox.run(&["rename", "2", "   "]);
    assert_eq!(out.status.code(), Some(1), "{}", combined(&out));
    assert!(stderr(&out).contains("cannot be empty"), "{}", stderr(&out));

    let out = sandbox.run(&["rename", "9", "x"]);
    assert_eq!(out.status.code(), Some(1), "{}", combined(&out));
    // The number is rejected by target resolution, with the range spelled out.
    assert!(stderr(&out).contains("out of range"), "{}", stderr(&out));
    assert!(stderr(&out).contains("use 1..2"), "{}", stderr(&out));

    assert_eq!(order(&sandbox), vec!["alpha", "2-second"], "refused renames change nothing");
}

#[test]
fn a_new_entry_beyond_max_display_warns_and_stays_hidden() {
    let mut sandbox = Sandbox::new("hidden-add");
    sandbox.add_dir("first");
    let mut cfg = sandbox.config();
    cfg.set_max_display(1).unwrap();
    cfg.save(&sandbox.config).unwrap();

    let second = sandbox.make_dir("second");
    let out = sandbox.run(&["add", second.to_str().unwrap()]);
    assert_eq!(out.status.code(), Some(0), "{}", combined(&out));
    let text = stdout(&out);
    assert!(text.starts_with("added [2] "), "{text}");
    assert!(text.contains("past \"max locations to display\""), "{text}");
    assert!(text.contains("scd mv 2 1"), "{text}");

    let menu = combined(&sandbox.feed("q\n"));
    assert!(menu.contains("[1] "), "{menu}");
    assert!(!menu.contains("[2] "), "{menu}");
    assert!(menu.contains("2 saved, 1 hidden"), "{menu}");

    // Bringing it to the top makes it visible without touching the limit.
    let out = sandbox.run(&["mv", "2", "1"]);
    assert_eq!(out.status.code(), Some(0), "{}", combined(&out));
    let menu = combined(&sandbox.feed("q\n"));
    assert!(menu.contains(&abs(&second).display().to_string()), "{menu}");
}

#[test]
fn hand_edited_order_is_respected() {
    let mut sandbox = Sandbox::new("handmade-order");
    let alpha = sandbox.add_dir("alpha");
    let beta = sandbox.add_dir("beta");
    let gamma = sandbox.add_dir("gamma");

    // The user rearranges config.json by hand: gamma, alpha, beta.
    let mut cfg = sandbox.config();
    cfg.locations.rotate_left(2);
    cfg.save(&sandbox.config).unwrap();

    assert_eq!(stdout(&sandbox.run(&["1"])).trim(), abs(&gamma).display().to_string());
    assert_eq!(stdout(&sandbox.run(&["2"])).trim(), abs(&alpha).display().to_string());
    assert_eq!(stdout(&sandbox.run(&["3"])).trim(), abs(&beta).display().to_string());
    let text = stdout(&sandbox.run(&["list"]));
    assert!(text.find("gamma").unwrap() < text.find("alpha").unwrap(), "{text}");
}
