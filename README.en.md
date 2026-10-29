[中文](README.md) | **English**

# scd — super cd

A favourites folder for your terminal: type `scd`, press a number, land in the directory you
actually meant. No more retyping long paths.

```console
PS D:\> scd

    [c] configure scd
    [a] add new location

    [1] ZETAmine  D:\data\ZETAmine
    [2] dl        D:\browserDownload

    [q] quit

scd> 2
PS D:\browserDownload>
```

- **Nothing to install first**: a single exe with zero dependencies — no Rust, no Python, no admin rights
- **Numbers never move**: the order and the aliases are entirely yours; scd never reorders by usage
- **The config is plain JSON**: `config.json` can be edited in Notepad

> ## Platform support status
>
> **Windows x64 is the only officially published platform, and the only one the author has
> verified by hand on a real machine** (download → extract → double-click `install.cmd` →
> open a new terminal → `scd` really changes the directory: the whole flow has been walked
> end to end).
>
> The Linux / macOS build scripts are still in the repository and you are welcome to build
> for yourself, but **no official packages are published for them and they have not been
> verified by anyone**. CI builds Windows x64 only.

---

# Part 1 · User guide (Windows, ready to run)

## 1. Download

Open the **[Releases page](https://github.com/WR009/scd/releases)** and grab these two files
from the latest release:

| File | What it is |
| --- | --- |
| `scd-<version>-x86_64-pc-windows-msvc.zip` | The package. Extract it, then double-click `install.cmd` inside |
| `scd-<version>-x86_64-pc-windows-msvc.zip.sha256` | Checksum (optional) |

Current version: **0.2.1**. See [CHANGELOG.md](CHANGELOG.md) for what changed.

Optional integrity check:

```powershell
certutil -hashfile scd-0.2.1-x86_64-pc-windows-msvc.zip SHA256
```

Compare the result with the contents of the `.sha256` file.

## 2. Install (three steps)

1. **Download** the `.zip`
2. **Extract** it anywhere (right-click → Extract All)
3. **Double-click `install.cmd`** in the extracted folder

It first asks where to install:

```console
  scd installer
  source : D:\downloads\scd-0.2.1-x86_64-pc-windows-msvc\

  install into (press Enter for C:\Users\you\scd) : D:\applications\scd
  target : D:\applications\scd
```

**Press Enter** to install into `%USERPROFILE%\scd`, or type a full path (for example
`D:\applications\scd`) and then press Enter.

When you see `Done. Open a NEW terminal and type:  scd` you are finished. **Open a new
terminal** and type `scd`.

`install.cmd` does all of the following for you — no environment variable has to be set by hand:

| What it does | Detail |
| --- | --- |
| Installs the program | Copies to the directory you chose, e.g. `D:\applications\scd\bin\scd.exe` |
| Extends PATH | Adds `...\scd\bin` **and** `...\scd\shell` to the *user* PATH (no admin needed), with `shell` placed **ahead of** `bin` — cmd.exe requires that order |
| Makes `scd` really change directory | Appends `. "...\scd\shell\scd.ps1"` to the PowerShell `$PROFILE` |

> **Prefer one command without the prompt?** From the extracted folder:
>
> ```powershell
> powershell -NoProfile -ExecutionPolicy Bypass -File .\install.ps1 -Dest D:\applications\scd -AddToPath -AddToProfile
> ```
>
> Passing any argument to `install.cmd` also skips the prompt: `install.cmd -Dest D:\applications\scd`

### What the installed folder looks like

```
D:\applications\scd\
├── bin\scd.exe          the program
├── shell\               the scd function for each shell (scd.ps1 / scd.cmd / scd.bash / scd.zsh / scd.fish)
├── config.json          your saved locations (editable in Notepad)
├── config.example.json  an example config
├── README.md  README.en.md  LICENSE  CHANGELOG.md
└── install.cmd  install.ps1  install.sh
```

**Upgrading**: extract the new zip and double-click `install.cmd` again, choosing the same
directory. Your `config.json` is kept as it is.

## 3. What to do in each terminal

| Terminal | Anything else to do? | How it works |
| --- | --- | --- |
| **PowerShell** | Nothing — `install.cmd` already wrote `$PROFILE` | the `scd` function from `$PROFILE` performs the real `cd` |
| **cmd.exe** | Nothing — `install.cmd` already put `shell` on PATH | it runs the `shell\scd.cmd` batch wrapper |
| **Git Bash** | Add two lines to `~/.bashrc` (below) | batch files and PowerShell functions do not apply to bash |

For Git Bash, add these lines (adjust the path to your installation):

```bash
export PATH="/d/applications/scd/bin:$PATH"
source "/d/applications/scd/shell/scd.bash"
```

> **Why is a shell function needed at all?** No child process can change the working
> directory of the shell that started it. So `scd.exe` resolves the target directory and
> hands it back, and the function in `shell\` performs the actual `cd`. Once installed,
> `scd` behaves exactly like `cd`. Without the function, `scd` still tells you where to go
> and prints a ready-to-paste `cd "..."` line.

## 4. Everyday use

### The menu: `scd`

| Key | Action |
| --- | --- |
| `1` `2` `3` … | jump to that location |
| `a` | add a location (type a path, `here` for the current one, `quit` to cancel) |
| `c` | settings (how many locations the menu lists, and more) |
| `q` | quit |
| anything else | treated as a target, so you can paste a path at the prompt |

### Jump without the menu: `scd <target>`

Targets are tried in this order:

1. **A directory path** — `scd D:\code`, `scd ~\code`, `scd ..\docs`, `scd "C:\my dir"`, `scd .`
2. **A menu number** — `scd 3` (the number shown by `scd list`)
3. **A saved name** — `scd proj` (exact, prefix, substring, path or fuzzy match)

### Managing your saved locations

| Command | What it does |
| --- | --- |
| `scd add .` | Save the current directory (**appended at the end**, existing numbers stay valid) |
| `scd add "D:\my dir"` | Save a specific directory |
| `scd rm <target>` | Forget a location |
| `scd rename <target> <new name>` | Change the alias; spaces and non-ASCII are fine |
| `scd mv <target> <n>` | Put it at position n |
| `scd up <target>` / `scd down <target>` | Move it one step up / down |
| `scd list` | List everything (including visit statistics) |
| `scd config` | Show which `config.json` is in use |
| `scd --help` | The full user guide |

## 5. Order and aliases are yours

scd **never** reorders the list by usage:

- New locations are always **appended**, so numbers you already know stay valid
- Rearrange with `scd mv / up / down`, or edit the order of the `locations` array in
  `config.json` directly in Notepad (**array order = menu order**)
- `visits` and `last_used_at` are statistics only; `scd list` shows them, but they never
  influence the order

## 6. The config file

By default it lives at `<install dir>\config.json` and can be edited in Notepad. Every write
is atomic (a temporary file is written first and then renamed), so the file cannot be left half-written.

```json
{
  "version": 1,
  "max_display": 5,
  "locations": [
    {
      "name": "ZETAmine",
      "path": "D:\\data\\ZETAmine",
      "visits": 6,
      "added_at": "2026-10-27T13:40:19Z",
      "last_used_at": "2026-10-29T02:50:03Z"
    }
  ]
}
```

`max_display` is exactly what `[c] configure scd` in the menu edits.

> Lookup order: `--config` → `%SCD_CONFIG%` → `%SCD_HOME%\config.json` → the parent of the
> exe when it sits in `bin\` → the directory of the exe → `~/.scd/config.json`

## 7. Colour

In a real terminal the output is coloured: aliases green, paths cyan, key hints yellow,
errors red, secondary information dim. Colour switches itself off when the output is piped or
redirected, so scripts always receive clean text.

| Goal | How |
| --- | --- |
| Force on / off | `$env:SCD_COLOR='always'` / `$env:SCD_COLOR='never'` |
| Follow the common convention | set `NO_COLOR=1` |

## 8. Uninstall

1. Delete the installation directory (e.g. `D:\applications\scd`)
2. Remove `...\scd\bin` and `...\scd\shell` from the user PATH
3. Remove the `. "...\shell\scd.ps1"` line from `$PROFILE` (and the two lines from `~/.bashrc` if you use Git Bash)

## 9. Troubleshooting

| Symptom | Cause and fix |
| --- | --- |
| `'scd' is not recognized` / `scd : The term 'scd' is not recognized` | PATH was changed at install time — **open a new terminal**; or check that `...\scd\bin` is in the user PATH |
| It prints the directory but does not move, with `scd cannot change this shell's directory by itself` | The shell function is not loaded. Check that `$PROFILE` contains `. "<install dir>\shell\scd.ps1"` (`install.cmd` writes it for you) |
| PowerShell says scripts are disabled on this system | The execution policy is Restricted, so `$PROFILE` never runs. Once, no admin needed: `Set-ExecutionPolicy -Scope CurrentUser RemoteSigned` |
| cmd.exe prints the directory but does not change to it | `shell` must come **before** `bin` on PATH (cmd.exe prefers `.exe` over `.cmd` in one directory). Installing with `install.cmd` gets the order right |
| Git Bash: `source scd.bash` fails with `$'\r': command not found` | You have the old 0.2.0 package, whose shell scripts were checked out with CRLF. Use 0.2.1 or newer |
| Just want it once, without installing a shell function | `cd "$(scd proj)"` (works in PowerShell and Git Bash) |
| No colour, please | `SCD_COLOR=never` or `NO_COLOR=1` |
| `install.cmd` fails or the window closes at once | Run `install.ps1` from PowerShell to see the full error, and check that the destination is writable (`D:\applications` sometimes needs an explicit grant) |

---

# Part 2 · Building from source (developers)

> If you only want to use scd, skip this part: the Release already has a built package.
> A Rust toolchain is only needed to change the code.

## 1. Requirements

- Windows 10/11 x64 plus Rust **stable** (`edition 2024`, i.e. 1.85 or newer) via [rustup](https://rustup.rs)
- **Zero third-party dependencies**: `[dependencies]` in `Cargo.toml` is empty. The JSON parser,
  UTC time, ANSI colour and the Windows VT enablement are all hand-written, so a clean checkout
  **builds offline** and `--locked` in CI never downloads anything

## 2. Build and test

```powershell
cargo build --release   # -> target\release\scd.exe
cargo test              # 160 tests: 110 unit + 50 end-to-end
cargo clippy --all-targets   # optional
```

## 3. Installing from the repository

`install.ps1` in the repository is **the same script** that ships in the release package: it
detects whether it is running inside a source checkout (there is a `Cargo.toml`, so it builds
first) or inside an extracted archive (there is a `bin\scd.exe`, so it just installs).

```powershell
.\install.ps1 -Dest D:\applications\scd -AddToPath -AddToProfile
```

## 4. Code layout

```
src/
├── main.rs      entry point
├── cli.rs       argument parsing and subcommand dispatch
├── lib.rs       library root (used by the tests)
├── config.rs    config.json read/write: atomic replace + structure validation
├── json.rs      a hand-written minimal JSON parser / serialiser
├── time.rs      hand-written UTC time (no chrono)
├── color.rs     ANSI colour, tty detection, Windows VT enablement, colour policy
├── paths.rs     config lookup order, path normalisation and comparison
├── menu.rs      the interactive menu
├── jump.rs      target resolution: path / number / name (five matching levels)
├── add.rs       add / rm / mv / up / down / rename
├── emit.rs      output channels: human-readable vs machine-readable (the latter is never coloured)
├── settings.rs  the items behind `[c] configure`
└── ui.rs        terminal rendering
shell/           the scd function / wrapper for five shells
tools/dist.ps1   the packaging script (Windows; this is what CI runs)
tools/dist.sh    the packaging script (Linux / macOS; self-build only)
tools/preview.ps1  renders a colour preview to target/preview.html
tests/cli.rs     end-to-end tests
docs/            ARCHITECTURE.md (specification), DESIGN.md (design decisions) — both in Chinese
```

## 5. Packaging and releasing

`.github/workflows/release.yml` builds **Windows x64 only**: `cargo test --locked` →
`cargo build --release --locked` → package with `tools/dist.ps1` → smoke-test the exe inside
the package → upload to the GitHub Release.

```powershell
# 1) bump the version in Cargo.toml and write CHANGELOG.md
# 2) commit and tag
git tag -a v0.3.0 -m "scd 0.3.0"
git push origin master --tags
```

To produce a package locally without publishing anything:

```powershell
.\tools\dist.ps1 -Target x86_64-pc-windows-msvc -Label Windows_x64 -Ext zip
```

Output lands in `releases\` (gitignored, never committed): `scd-<version>-<target>.zip` plus
`.sha256`, and an already-unpacked `releases\Windows_x64\` so you can test the install flow.

The packaging script has three guards: it fails if any packaged `.sh/.bash/.zsh/.fish` file
contains a CR (`.gitattributes` pins `eol=lf` for them), if the zip lacks the `PK` magic, or if
a `.tar.gz` is not really gzip — so a broken archive cannot be shipped by accident.

## 6. Other platforms

`install.sh`, `tools/dist.sh` and `shell/scd.bash|zsh|fish` remain in the repository, so you
can build and install on Linux or macOS yourself. **No official packages are published for
those platforms and nobody has verified them**; CI no longer builds them. To do it yourself:

```bash
cargo build --release --target x86_64-unknown-linux-gnu
./tools/dist.sh --target x86_64-unknown-linux-gnu --label Linux_x64 --ext tar.gz
bash install.sh --dest ~/.local/scd --add-to-rc
```

## 7. Full reference

**Environment variables**

| Variable | Purpose |
| --- | --- |
| `SCD_CONFIG=<file>` | the config file to use |
| `SCD_HOME=<dir>` | the install directory holding `bin\` and `config.json` |
| `SCD_BIN=<exe>` | the binary the shell function should call |
| `SCD_TARGET_FILE=<file>` | write the chosen directory to this file (this is how the shell wrappers get the result) |
| `SCD_CD=1` | the caller is a shell function: suppress the install hint |
| `SCD_NO_HINT=1` | never print the install hint |
| `SCD_COLOR=auto\|always\|never` | colour policy; the default `auto` colours only a real terminal |
| `NO_COLOR=1` | same as `SCD_COLOR=never` |
| `CLICOLOR_FORCE=1` | colour even when the output is redirected |

**Exit codes**

| Code | Meaning |
| --- | --- |
| 0 | success (quitting the menu without jumping counts as success) |
| 1 | the target could not be resolved, or it is not a directory |
| 2 | `config.json` could not be read, parsed or written |
| 64 | the command line itself was wrong |

## 8. More documentation

- [README.md](README.md) —— 中文版
- [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) —— the functional specification (Chinese)
- [docs/DESIGN.md](docs/DESIGN.md) —— design decisions: why zero dependencies, why no usage-based reordering, the colour policy
- [CHANGELOG.md](CHANGELOG.md) —— version history

## License

MIT © 2026 River
