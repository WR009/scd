//! `config.json` - the only state `scd` keeps: saved locations plus display settings.
//!
//! On-disk shape (hand-editable, unknown keys are ignored so older builds keep working):
//!
//! ```json
//! {
//!   "version": 1,
//!   "max_display": 9,
//!   "locations": [
//!     {
//!       "name": "scd",
//!       "path": "D:\\data\\Projects\\scd",
//!       "visits": 3,
//!       "added_at": "2026-10-27T09:00:00Z",
//!       "last_used_at": "2026-10-27T09:30:00Z"
//!     }
//!   ]
//! }
//! ```
//!
//! Locations are kept most-recently-used first: the menu lists the first `max_display`
//! entries, so the directories you actually use stay on top.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::json::Json;
use crate::paths;
use crate::time;

/// Schema version written by this build.
pub const VERSION: i64 = 1;
/// Value used when `max_display` is missing or unusable.
pub const DEFAULT_MAX_DISPLAY: usize = 9;
pub const MIN_MAX_DISPLAY: usize = 1;
pub const MAX_DISPLAY_LIMIT: usize = 99;
/// File name inside the install directory (`<install>/config.json`).
pub const FILE_NAME: &str = "config.json";

/// One saved directory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Location {
    /// Friendly label used in the menu and for `scd <name>`; defaults to the folder name.
    pub name: String,
    /// Absolute, normalised path.
    pub path: PathBuf,
    pub visits: u64,
    pub added_at: i64,
    pub last_used_at: i64,
}

impl Location {
    pub fn new(path: PathBuf) -> Self {
        let now = time::unix_now();
        Location {
            name: paths::file_name(&path),
            path,
            visits: 0,
            added_at: now,
            last_used_at: now,
        }
    }

    /// One-line rendering used outside the aligned menu, e.g. `scd  D:\data\Projects\scd`.
    pub fn summary(&self) -> String {
        if self.name.is_empty() {
            self.path.display().to_string()
        } else {
            format!("{}  {}", self.name, self.path.display())
        }
    }
}

/// Result of [`Config::add`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Added {
    /// Appended at this index (the end of the list, so existing numbers never shift).
    New(usize),
    /// Already saved; holds the existing index.
    Existing(usize),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    /// How many locations the menu lists ("显示多少个位置").
    pub max_display: usize,
    /// Saved locations, most recently used first.
    pub locations: Vec<Location>,
}

impl Default for Config {
    fn default() -> Self {
        Config { max_display: DEFAULT_MAX_DISPLAY, locations: Vec::new() }
    }
}

impl Config {
    pub fn is_empty(&self) -> bool {
        self.locations.is_empty()
    }

    /// Entries the menu may show.
    pub fn shown(&self) -> &[Location] {
        let end = self.locations.len().min(self.max_display);
        &self.locations[..end]
    }

    /// Entries that exist but are hidden by `max_display`.
    pub fn hidden_count(&self) -> usize {
        self.locations.len().saturating_sub(self.max_display)
    }

    pub fn set_max_display(&mut self, value: usize) -> Result<(), String> {
        if !(MIN_MAX_DISPLAY..=MAX_DISPLAY_LIMIT).contains(&value) {
            return Err(format!(
                "{value} is out of range: please enter a whole number between {MIN_MAX_DISPLAY} and {MAX_DISPLAY_LIMIT}"
            ));
        }
        self.max_display = value;
        Ok(())
    }

    /// Save a directory; duplicates are reported instead of stored twice.
    ///
    /// New entries are **appended**: the order of `locations` belongs to the user, so adding
    /// something must never renumber the menu. Use [`Config::move_to`] / [`Config::shift`]
    /// (or edit `config.json`) to arrange them.
    pub fn add(&mut self, path: PathBuf) -> Added {
        let resolved = paths::absolute(&path);
        if let Some(index) = self.index_of_path(&resolved) {
            return Added::Existing(index);
        }
        let mut location = Location::new(resolved);
        self.make_name_unique(&mut location);
        self.locations.push(location);
        Added::New(self.locations.len() - 1)
    }

    /// Find a saved location by path (platform-appropriate comparison).
    pub fn index_of_path(&self, path: &Path) -> Option<usize> {
        let resolved = paths::absolute(path);
        self.locations.iter().position(|location| paths::same_path(&location.path, &resolved))
    }

    /// Count a visit.
    ///
    /// Deliberately does **not** reorder: usage statistics are metadata for `scd list`, not a
    /// ranking. The menu order only changes when the user asks for it.
    pub fn record_visit(&mut self, index: usize) {
        if let Some(location) = self.locations.get_mut(index) {
            location.visits = location.visits.saturating_add(1);
            location.last_used_at = time::unix_now();
        }
    }

    pub fn remove(&mut self, index: usize) -> Location {
        self.locations.remove(index)
    }

    /// Move a location to another 0-based position. Returns its new index.
    pub fn move_to(&mut self, from: usize, to: usize) -> Result<usize, String> {
        let count = self.locations.len();
        if from >= count {
            return Err(no_such_index(from));
        }
        if to >= count {
            return Err(format!(
                "position {} does not exist: you have {count} location(s), so use 1..{count}",
                to + 1
            ));
        }
        if from == to {
            return Ok(from);
        }
        let location = self.locations.remove(from);
        self.locations.insert(to, location);
        Ok(to)
    }

    /// Move one step towards the top (`delta < 0`) or the bottom (`delta > 0`), clamped.
    pub fn shift(&mut self, index: usize, delta: i32) -> Result<usize, String> {
        let count = self.locations.len();
        if index >= count {
            return Err(no_such_index(index));
        }
        let target = ((index as i64) + (delta as i64)).clamp(0, (count - 1) as i64) as usize;
        self.move_to(index, target)
    }

    /// Change a location's alias. Empty names and names taken by another location are refused.
    pub fn rename(&mut self, index: usize, name: &str) -> Result<(), String> {
        let cleaned = name.trim();
        if cleaned.is_empty() {
            return Err("the new name cannot be empty".to_string());
        }
        if cleaned.chars().count() > 60 {
            return Err("the new name is too long (60 characters max)".to_string());
        }
        let folded = cleaned.to_lowercase();
        if let Some(other) = self.locations.iter().position(|l| l.name.to_lowercase() == folded) {
            if other != index {
                return Err(format!(
                    "\"{cleaned}\" is already used by location [{}]; pick another name",
                    other + 1
                ));
            }
        }
        match self.locations.get_mut(index) {
            Some(location) => {
                location.name = cleaned.to_string();
                Ok(())
            }
            None => Err(no_such_index(index)),
        }
    }

    /// Match a typed name against saved locations, strongest tier first:
    /// exact name, name prefix, name substring, folder-component prefix, fuzzy subsequence.
    pub fn match_name(&self, query: &str) -> Vec<usize> {
        let needle = query.trim().to_lowercase();
        if needle.is_empty() {
            return Vec::new();
        }
        let names: Vec<String> = self.locations.iter().map(|l| l.name.to_lowercase()).collect();
        // Folder components only: matching the whole path string would let a stray digit
        // or a user name in `C:\Users\...` hijack short queries.
        let components: Vec<Vec<String>> = self
            .locations
            .iter()
            .map(|location| {
                location
                    .path
                    .components()
                    .filter_map(|component| component.as_os_str().to_str())
                    .map(|part| part.to_lowercase())
                    .collect()
            })
            .collect();

        let exact: Vec<usize> = indices(&names, |name| *name == needle);
        if !exact.is_empty() {
            return exact;
        }
        let prefix: Vec<usize> = indices(&names, |name| name.starts_with(&needle));
        if !prefix.is_empty() {
            return prefix;
        }
        let substring: Vec<usize> = indices(&names, |name| name.contains(&needle));
        if !substring.is_empty() {
            return substring;
        }
        let by_path: Vec<usize> = components
            .iter()
            .enumerate()
            .filter(|(_, parts)| parts.iter().any(|part| part.starts_with(&needle)))
            .map(|(index, _)| index)
            .collect();
        if !by_path.is_empty() {
            return by_path;
        }
        indices(&names, |name| is_subsequence(&needle, name))
    }

    fn make_name_unique(&mut self, location: &mut Location) {
        let base = location.name.clone();
        if base.is_empty() || !self.has_name(&base) {
            return;
        }
        if let Some(parent) = location.path.parent().and_then(|parent| parent.file_name()) {
            let candidate = format!("{} ({})", base, parent.to_string_lossy());
            if !self.has_name(&candidate) {
                location.name = candidate;
                return;
            }
        }
        for suffix in 2..1000 {
            let candidate = format!("{base} ({suffix})");
            if !self.has_name(&candidate) {
                location.name = candidate;
                return;
            }
        }
    }

    fn has_name(&self, name: &str) -> bool {
        let folded = name.to_lowercase();
        self.locations.iter().any(|location| location.name.to_lowercase() == folded)
    }

    /// Read a config file. A missing file is not an error: it means "fresh install".
    pub fn load(path: &Path) -> Result<Config, String> {
        let text = match fs::read_to_string(path) {
            Ok(text) => text,
            Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(Config::default()),
            Err(err) => return Err(format!("cannot read {}: {}", path.display(), io_err(&err))),
        };
        // PowerShell writes a BOM by default; tolerate it instead of failing.
        let text = text.strip_prefix('\u{feff}').unwrap_or(&text);
        if text.trim().is_empty() {
            return Ok(Config::default());
        }
        let json = Json::parse(text).map_err(|err| format!("cannot parse {}: {err}", path.display()))?;
        let base = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("."));
        Config::from_json(&json, &base).map_err(|err| format!("cannot use {}: {err}", path.display()))
    }

    /// Write atomically: a crash mid-save can only lose the temporary file, never the config.
    pub fn save(&self, path: &Path) -> Result<(), String> {
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() && !parent.is_dir() {
                fs::create_dir_all(parent)
                    .map_err(|err| format!("cannot create {}: {}", parent.display(), io_err(&err)))?;
            }
        }
        let mut text = self.to_json().to_string_pretty();
        text.push('\n');

        let name = path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| FILE_NAME.to_string());
        let tmp = path.with_file_name(format!("{name}.tmp"));
        fs::write(&tmp, text.as_bytes())
            .map_err(|err| format!("cannot write {}: {}", tmp.display(), io_err(&err)))?;
        fs::rename(&tmp, path).map_err(|err| {
            let _ = fs::remove_file(&tmp);
            format!("cannot save {}: {}", path.display(), io_err(&err))
        })?;
        Ok(())
    }

    pub fn from_json(json: &Json, base: &Path) -> Result<Config, String> {
        if json.as_object().is_none() {
            return Err("the top level value must be an object".to_string());
        }
        let max_display = json
            .get("max_display")
            .or_else(|| json.get("maxDisplay"))
            .or_else(|| json.get("display"))
            .and_then(Json::as_i64)
            .map(clamp_display)
            .unwrap_or(DEFAULT_MAX_DISPLAY);
        let mut config = Config { max_display, locations: Vec::new() };

        let raw = match json.get("locations").or_else(|| json.get("paths")) {
            Some(Json::Null) | None => return Ok(config),
            Some(value) => value,
        };
        let items = raw.as_array().ok_or("\"locations\" must be an array")?;
        for item in items {
            if let Some(location) = location_from_json(item, base)? {
                if config.index_of_path(&location.path).is_none() {
                    config.locations.push(location);
                }
            }
        }
        Ok(config)
    }

    pub fn to_json(&self) -> Json {
        let mut root = Json::object();
        root.insert("version", Json::from(VERSION));
        root.insert("max_display", Json::from(self.max_display));
        let mut locations = Json::array();
        for location in &self.locations {
            let mut entry = Json::object();
            entry.insert("name", Json::from(location.name.clone()));
            entry.insert("path", Json::from(location.path.to_string_lossy().into_owned()));
            entry.insert("visits", Json::from(location.visits));
            entry.insert("added_at", Json::from(time::format_utc(location.added_at)));
            entry.insert("last_used_at", Json::from(time::format_utc(location.last_used_at)));
            locations.push(entry);
        }
        root.insert("locations", locations);
        root
    }
}

/// One-line rendering of an I/O error for user-facing messages.
fn io_err(err: &io::Error) -> String {
    crate::ui::flatten(&err.to_string())
}

fn no_such_index(index: usize) -> String {
    format!("there is no location [{}]", index + 1)
}

fn indices(values: &[String], matches: impl Fn(&str) -> bool) -> Vec<usize> {
    values.iter().enumerate().filter(|(_, value)| matches(value)).map(|(index, _)| index).collect()
}

fn is_subsequence(needle: &str, haystack: &str) -> bool {
    let mut chars = haystack.chars();
    needle.chars().all(|want| chars.by_ref().any(|ch| ch == want))
}

fn clamp_display(value: i64) -> usize {
    if value < MIN_MAX_DISPLAY as i64 {
        MIN_MAX_DISPLAY
    } else if value > MAX_DISPLAY_LIMIT as i64 {
        MAX_DISPLAY_LIMIT
    } else {
        value as usize
    }
}

fn location_from_json(item: &Json, base: &Path) -> Result<Option<Location>, String> {
    let (raw_path, name, visits, added_at, last_used_at) = match item {
        // Shorthand: a bare string is just a path.
        Json::Str(text) => (text.as_str(), None, 0u64, 0i64, 0i64),
        Json::Object(_) => (
            item.get("path")
                .or_else(|| item.get("dir"))
                .or_else(|| item.get("location"))
                .and_then(Json::as_str)
                .unwrap_or(""),
            item.get("name").and_then(Json::as_str),
            item.get("visits").and_then(Json::as_u64).unwrap_or(0),
            stamp(item.get("added_at")),
            stamp(item.get("last_used_at")),
        ),
        _ => return Ok(None),
    };

    let text = paths::unquote(raw_path.trim());
    if text.is_empty() {
        return Ok(None);
    }
    // Stored relative paths are resolved against the config file, not against the shell's
    // current directory, so they keep meaning the same thing tomorrow.
    let path = paths::absolute_from(&PathBuf::from(text), base);
    if path.as_os_str().is_empty() {
        return Ok(None);
    }
    let name = name
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| paths::file_name(&path));
    Ok(Some(Location { name, path, visits, added_at, last_used_at }))
}

fn stamp(value: Option<&Json>) -> i64 {
    match value {
        Some(Json::Str(text)) => time::parse_utc(text).unwrap_or(0),
        Some(other) => other.as_i64().unwrap_or(0),
        None => 0,
    }
}

/// Which `config.json` this run should use.
///
/// Order: `--config` > `$SCD_CONFIG` > `$SCD_HOME/config.json` > the documented install
/// layout (`<install>/bin/scd` -> `<install>/config.json`) > a file next to the executable
/// > `~/.scd/config.json`.
pub fn resolve_path(cli_override: Option<&Path>) -> PathBuf {
    if let Some(path) = cli_override {
        return path.to_path_buf();
    }
    if let Some(path) = env_path("SCD_CONFIG") {
        return path;
    }
    if let Some(root) = env_path("SCD_HOME") {
        return root.join(FILE_NAME);
    }
    if let Some(path) = exe_config_path() {
        return path;
    }
    match paths::home_dir() {
        Some(home) => home.join(".scd").join(FILE_NAME),
        None => PathBuf::from(FILE_NAME),
    }
}

fn env_path(key: &str) -> Option<PathBuf> {
    let value = std::env::var_os(key)?;
    let text = value.to_string_lossy().into_owned();
    let path = PathBuf::from(paths::unquote(&text));
    if path.as_os_str().is_empty() {
        None
    } else {
        Some(path)
    }
}

fn exe_config_path() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    path_from_exe_dir(exe.parent()?)
}

/// The `bin/` rule from docs/ARCHITECTURE.md, exposed for tests.
pub fn path_from_exe_dir(exe_dir: &Path) -> Option<PathBuf> {
    let is_bin = exe_dir
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.eq_ignore_ascii_case("bin"));
    if is_bin {
        return exe_dir.parent().map(|root| root.join(FILE_NAME));
    }
    let sibling = exe_dir.join(FILE_NAME);
    if sibling.is_file() {
        return Some(sibling);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn workspace(tag: &str) -> PathBuf {
        let nanos = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let dir = std::env::temp_dir().join(format!("scd-cfg-{}-{tag}-{nanos}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        // CI runners hand out temp dirs that are not in canonical form (8.3 short names
        // on the Windows runners, /var -> /private/var on macOS). scd reports canonical
        // paths, so the sandbox root has to be canonical too or the two spellings of the
        // same directory never compare equal.
        crate::paths::absolute(&dir)
    }

    fn cleanup(dir: &Path) {
        let _ = fs::remove_dir_all(dir);
    }

    fn sample(dir: &Path, name: &str) -> Location {
        let path = paths::absolute(&dir.join(name));
        fs::create_dir_all(&path).unwrap();
        let mut location = Location::new(path);
        location.name = name.to_string();
        location
    }

    #[test]
    fn missing_file_loads_as_default() {
        let dir = workspace("missing");
        let config = Config::load(&dir.join(FILE_NAME)).unwrap();
        assert_eq!(config, Config::default());
        assert_eq!(config.max_display, DEFAULT_MAX_DISPLAY);
        cleanup(&dir);
    }

    #[test]
    fn blank_and_bom_only_file_loads_as_default() {
        let dir = workspace("blank");
        let path = dir.join(FILE_NAME);
        fs::write(&path, "\u{feff}   \n").unwrap();
        assert_eq!(Config::load(&path).unwrap(), Config::default());
        cleanup(&dir);
    }

    #[test]
    fn corrupt_file_is_reported_and_left_alone() {
        let dir = workspace("corrupt");
        let path = dir.join(FILE_NAME);
        fs::write(&path, "{ oops").unwrap();
        let err = Config::load(&path).unwrap_err();
        assert!(err.contains("cannot parse"), "{err}");
        assert!(err.contains(&path.display().to_string()), "{err}");
        assert_eq!(fs::read_to_string(&path).unwrap(), "{ oops");
        cleanup(&dir);
    }

    #[test]
    fn wrong_shapes_are_reported() {
        let dir = workspace("shape");
        assert!(Config::from_json(&Json::parse("[1,2]").unwrap(), &dir).is_err());
        assert!(
            Config::from_json(&Json::parse(r#"{"locations": 5}"#).unwrap(), &dir)
                .unwrap_err()
                .contains("\"locations\" must be an array")
        );
        cleanup(&dir);
    }

    #[test]
    fn parses_aliases_shorthand_and_clamps_display() {
        let dir = workspace("aliases");
        let a = sample(&dir, "alpha");
        let b = sample(&dir, "beta");
        let text = format!(
            r#"{{
                 "maxDisplay": 0,
                 "locations": [
                   {{ "path": {:?}, "name": "ALPHA", "visits": 4, "added_at": "2026-01-02T03:04:05Z" }},
                   {:?},
                   {{ "nopath": true }},
                   ""
                 ]
               }}"#,
            a.path.to_string_lossy().into_owned(),
            b.path.to_string_lossy().into_owned()
        );
        let config = Config::from_json(&Json::parse(&text).unwrap(), &dir).unwrap();
        assert_eq!(config.max_display, MIN_MAX_DISPLAY, "0 is clamped up");
        assert_eq!(config.locations.len(), 2);
        assert_eq!(config.locations[0].name, "ALPHA");
        assert_eq!(config.locations[0].visits, 4);
        assert_eq!(config.locations[0].added_at, time::parse_utc("2026-01-02T03:04:05Z").unwrap());
        assert_eq!(config.locations[1].name, "beta", "shorthand entries derive their name");
        cleanup(&dir);
    }

    #[test]
    fn duplicate_paths_are_deduplicated_on_load() {
        let dir = workspace("dedupe");
        let a = sample(&dir, "alpha");
        let escaped = a.path.to_string_lossy().replace('\\', "\\\\").replace('"', "\\\"");
        let text = format!(r#"{{ "locations": [ {{ "path": "{escaped}" }}, {{ "path": "{escaped}" }} ] }}"#);
        let config = Config::from_json(&Json::parse(&text).unwrap(), &dir).unwrap();
        assert_eq!(config.locations.len(), 1);
        cleanup(&dir);
    }

    #[test]
    fn save_creates_parents_and_round_trips() {
        let dir = workspace("roundtrip");
        let path = dir.join("nested").join("deeper").join(FILE_NAME);
        let mut config = Config::default();
        config.add(sample(&dir, "one").path.clone());
        config.add(sample(&dir, "two").path.clone());
        config.set_max_display(3).unwrap();
        config.record_visit(1);

        config.save(&path).unwrap();
        let text = fs::read_to_string(&path).unwrap();
        assert!(text.starts_with("{\n  \"version\": 1,\n  \"max_display\": 3,"), "{text}");
        assert!(text.ends_with("}\n"), "file ends with a newline");
        assert!(!path.with_file_name(format!("{}.tmp", FILE_NAME)).exists(), "temp file is gone");

        let reloaded = Config::load(&path).unwrap();
        assert_eq!(reloaded.max_display, config.max_display);
        assert_eq!(reloaded.locations.len(), config.locations.len());
        assert_eq!(reloaded.locations, config.locations);
        cleanup(&dir);
    }

    #[test]
    fn add_reports_existing_locations_and_disambiguates_names() {
        let dir = workspace("add");
        let alpha = sample(&dir, "same");
        let beta = sample(&dir, "nested").path.join("same");
        fs::create_dir_all(&beta).unwrap();

        let mut config = Config::default();
        assert_eq!(config.add(alpha.path.clone()), Added::New(0));
        assert_eq!(config.add(alpha.path.clone()), Added::Existing(0));
        assert_eq!(config.locations.len(), 1);

        config.add(beta.clone());
        assert_eq!(config.locations.len(), 2);
        let names: Vec<&str> = config.locations.iter().map(|l| l.name.as_str()).collect();
        assert_eq!(names[0], "same", "appended: the first entry keeps its place");
        assert_eq!(names[1], "same (nested)", "{names:?}");
        cleanup(&dir);
    }

    fn names(config: &Config) -> Vec<String> {
        config.locations.iter().map(|location| location.name.clone()).collect()
    }

    #[test]
    fn add_appends_so_existing_numbers_never_shift() {
        let dir = workspace("append");
        let mut config = Config::default();
        assert_eq!(config.add(sample(&dir, "one").path.clone()), Added::New(0));
        assert_eq!(config.add(sample(&dir, "two").path.clone()), Added::New(1));
        assert_eq!(config.add(sample(&dir, "three").path.clone()), Added::New(2));
        assert_eq!(names(&config), vec!["one", "two", "three"]);
        cleanup(&dir);
    }

    #[test]
    fn record_visit_counts_without_reordering() {
        let dir = workspace("visit");
        let mut config = Config::default();
        config.add(sample(&dir, "one").path.clone());
        config.add(sample(&dir, "two").path.clone());
        config.add(sample(&dir, "three").path.clone());
        let before = names(&config);

        config.record_visit(2);
        config.record_visit(2);
        config.record_visit(0);
        assert_eq!(names(&config), before, "usage must never reorder the menu");
        assert_eq!(config.locations[2].visits, 2);
        assert_eq!(config.locations[0].visits, 1);
        assert_eq!(config.locations[1].visits, 0);
        assert!(config.locations[2].last_used_at > 0);
        config.record_visit(99); // out of range is a no-op, not a panic
        assert_eq!(names(&config), before);
        cleanup(&dir);
    }

    #[test]
    fn move_to_reorders_on_request() {
        let dir = workspace("move");
        let mut config = Config::default();
        for name in ["one", "two", "three", "four"] {
            config.add(sample(&dir, name).path.clone());
        }
        assert_eq!(names(&config), vec!["one", "two", "three", "four"]);

        assert_eq!(config.move_to(3, 0).unwrap(), 0);
        assert_eq!(names(&config), vec!["four", "one", "two", "three"]);
        assert_eq!(config.move_to(0, 3).unwrap(), 3);
        assert_eq!(names(&config), vec!["one", "two", "three", "four"]);
        assert_eq!(config.move_to(1, 1).unwrap(), 1, "no-op move");
        assert_eq!(names(&config), vec!["one", "two", "three", "four"]);

        assert!(config.move_to(0, 4).unwrap_err().contains("1..4"));
        assert!(config.move_to(9, 0).unwrap_err().contains("there is no location [10]"));
        cleanup(&dir);
    }

    #[test]
    fn shift_moves_one_step_and_clamps() {
        let dir = workspace("shift");
        let mut config = Config::default();
        for name in ["one", "two", "three"] {
            config.add(sample(&dir, name).path.clone());
        }
        assert_eq!(config.shift(0, 1).unwrap(), 1);
        assert_eq!(names(&config), vec!["two", "one", "three"]);
        assert_eq!(config.shift(1, -1).unwrap(), 0);
        assert_eq!(names(&config), vec!["one", "two", "three"]);

        assert_eq!(config.shift(0, -1).unwrap(), 0, "clamped at the top");
        assert_eq!(names(&config), vec!["one", "two", "three"]);
        assert_eq!(config.shift(2, 5).unwrap(), 2, "clamped at the bottom");
        assert!(config.shift(7, 1).is_err());
        cleanup(&dir);
    }

    #[test]
    fn rename_changes_the_alias_only() {
        let dir = workspace("rename");
        let mut config = Config::default();
        config.add(sample(&dir, "one").path.clone());
        config.add(sample(&dir, "two").path.clone());
        let path_before = config.locations[0].path.clone();

        config.rename(0, "  项目一  ").unwrap();
        assert_eq!(config.locations[0].name, "项目一", "trimmed and stored");
        assert_eq!(config.locations[0].path, path_before, "the path is untouched");
        assert_eq!(names(&config), vec!["项目一", "two"]);

        config.rename(0, "项目一").unwrap(); // renaming to its own name is fine
        assert!(config.rename(0, "two").unwrap_err().contains("already used by location [2]"));
        assert!(config.rename(0, "   ").unwrap_err().contains("cannot be empty"));
        assert!(config.rename(0, "").unwrap_err().contains("cannot be empty"));
        assert!(config.rename(0, &"x".repeat(61)).unwrap_err().contains("too long"));
        assert!(config.rename(9, "x").unwrap_err().contains("there is no location [10]"));
        assert_eq!(names(&config), vec!["项目一", "two"], "failed renames change nothing");
        cleanup(&dir);
    }

    #[test]
    fn match_name_walks_the_tiers_in_order() {
        let dir = workspace("match");
        let mut config = Config::default();
        for name in ["projects", "project-x", "docs", "工作目录"] {
            let path = dir.join(name);
            fs::create_dir_all(&path).unwrap();
            let mut location = Location::new(paths::absolute(&path));
            location.name = name.to_string();
            config.locations.push(location);
        }

        assert_eq!(config.match_name("docs"), vec![2]);
        assert_eq!(config.match_name("DOCS"), vec![2], "matching ignores case");
        assert_eq!(config.match_name("project"), vec![0, 1], "prefix tier");
        assert_eq!(config.match_name("工作"), vec![3], "unicode names work");
        assert_eq!(config.match_name("oject"), vec![0, 1], "substring tier");
        assert_eq!(config.match_name("dcs"), vec![2], "fuzzy subsequence tier");
        assert!(config.match_name("zzz").is_empty());
        assert!(config.match_name("   ").is_empty());
        cleanup(&dir);
    }

    #[test]
    fn match_name_finds_paths() {
        let dir = workspace("matchpath");
        let nested = dir.join("outer").join("inner");
        fs::create_dir_all(&nested).unwrap();
        let mut config = Config::default();
        config.add(nested.clone());
        config.locations[0].name = "unrelated".to_string();
        assert_eq!(config.match_name("outer"), vec![0]);
        cleanup(&dir);
    }

    #[test]
    fn shown_and_hidden_follow_max_display() {
        let dir = workspace("shown");
        let mut config = Config::default();
        for index in 0..5 {
            let path = dir.join(format!("d{index}"));
            fs::create_dir_all(&path).unwrap();
            config.locations.push(Location::new(paths::absolute(&path)));
        }
        config.set_max_display(2).unwrap();
        assert_eq!(config.shown().len(), 2);
        assert_eq!(config.hidden_count(), 3);
        config.set_max_display(50).unwrap();
        assert_eq!(config.shown().len(), 5);
        assert_eq!(config.hidden_count(), 0);
        cleanup(&dir);
    }

    #[test]
    fn set_max_display_validates() {
        let mut config = Config::default();
        assert!(config.set_max_display(0).is_err());
        assert!(config.set_max_display(MAX_DISPLAY_LIMIT + 1).is_err());
        assert!(config.set_max_display(1).is_ok());
        assert_eq!(config.max_display, 1);
        assert!(config.set_max_display(MAX_DISPLAY_LIMIT).is_ok());
    }

    #[test]
    fn to_json_writes_documented_keys() {
        let dir = workspace("tojson");
        let mut config = Config::default();
        config.add(sample(&dir, "alpha").path.clone());
        let json = config.to_json();
        assert_eq!(json.get("version").and_then(Json::as_i64), Some(VERSION));
        assert_eq!(json.get("max_display").and_then(Json::as_i64), Some(DEFAULT_MAX_DISPLAY as i64));
        let entry = &json.get("locations").and_then(Json::as_array).unwrap()[0];
        assert_eq!(entry.get("name").and_then(Json::as_str), Some("alpha"));
        assert!(entry.get("added_at").and_then(Json::as_str).unwrap().ends_with('Z'));
        cleanup(&dir);
    }

    #[test]
    fn install_layout_puts_config_next_to_bin() {
        let root = if cfg!(windows) { PathBuf::from(r"C:\tools\scd") } else { PathBuf::from("/opt/scd") };
        assert_eq!(path_from_exe_dir(&root.join("bin")), Some(root.join(FILE_NAME)));
        assert_eq!(path_from_exe_dir(&root.join("BIN")), Some(root.join(FILE_NAME)));
        // A plain directory only counts when a config.json already lives there.
        let dir = workspace("layout");
        assert_eq!(path_from_exe_dir(&dir), None);
        fs::write(dir.join(FILE_NAME), "{}").unwrap();
        assert_eq!(path_from_exe_dir(&dir), Some(dir.join(FILE_NAME)));
        cleanup(&dir);
    }

    #[test]
    fn resolve_path_prefers_the_cli_override() {
        let dir = workspace("resolve");
        let given = dir.join("custom.json");
        assert_eq!(resolve_path(Some(&given)), given);
        cleanup(&dir);
    }
}