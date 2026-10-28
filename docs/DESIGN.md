# scd 实现说明

本文记录 `docs/ARCHITECTURE.md` 的每条要求落在哪里、为什么这么设计，以及怎样验证。
面向要改代码的人；使用者请看 [`README.md`](../README.md)。

## 1. 需求对照表

| 需求（ARCHITECTURE.md） | 实现 | 验证 |
| --- | --- | --- |
| 基于 Rust，终端里快速跳转工作目录 | 零依赖 Rust 二进制 `scd`（lib + bin） | `cargo test`：160 项 |
| `scd` 直接启动弹出 `[c]/[a]/[1]..[n]` 菜单 | `src/menu.rs::menu_lines`（4 空格缩进、`[c]`/`[a]`/空行/编号列表） | `menu::tests::*`；`tests/cli.rs::bare_scd_shows_the_documented_menu` |
| `c` 设置最多显示多少个位置 n，并保留后续拓展 | `src/settings.rs`：`Setting` 枚举 + `ALL` 列表，逐项询问；当前只有 `MaxDisplay` | `settings::tests::*`；`tests/cli.rs::configure_flow_*` |
| 用了 `c` 就逐个询问需要修改的内容 | `settings::run` 遍历 `ALL`，每项一个 `[i/n] 标签 (提示; current: 值):` 问题；回车保持、`quit` 取消 | `configure_flow_sets_max_display`、`configure_flow_keeps_values_on_bare_enter_and_cancels_on_quit` |
| `a` 添加新的快速跳转地址 | `src/add.rs`（提示语与文档逐字一致） | `add::tests::*`；`tests/cli.rs::add_flow_follows_the_specification` |
| `a` 的提示：`type the new location, or type "here" ...` / `type "quit" to cancel` / `new location:` | `add::HEADING`、`add::PROMPT`（常量，便于比对） | `add_flow_follows_the_specification` 断言原文出现 |
| `"here"` 表示当前目录 | `add::is_here` → `paths::current_dir_absolute()` | `add_flow_understands_here` |
| `"quit"` 取消 | `add::is_cancel` → `Outcome::Cancel` | `add_flow_cancels_on_quit_without_touching_the_config` |
| 数字用于快速跳转 | `jump::resolve` → `Target::Saved(n-1)` | `jump::tests::resolves_numbers_and_names`；`tests/cli.rs::jump_by_path_number_and_name` |
| `scd --help` / `scd -h` 用户指南 | `src/cli.rs::HELP`（英文正文 + 中文速览） | `cli::tests::help_text_covers_the_specification`；`tests/cli.rs::help_prints_the_user_guide` |
| `scd [option]`：记得路径就直接用，不必打开 scd | `Command::Jump(query)`：路径 → 编号 → 名称 | `tests/cli.rs::jump_by_path_number_and_name`、`relative_paths_dot_segments_and_tilde_are_resolved` |
| 结构：`<位置>/scd/bin/` + `<位置>/scd/config.json` | `config::path_from_exe_dir`（exe 在 `bin/` 里 → 上一级的 `config.json`）；`install.ps1` / `install.sh` 生成该结构 | `config::tests::install_layout_puts_config_next_to_bin`；PowerShell 端到端 30 项检查 |
| 配置内容：地址、显示多少个位置、位置信息 | `config.json`：`max_display` + `locations[name,path,visits,added_at,last_used_at]` | `config::tests::to_json_writes_documented_keys`、`save_creates_parents_and_round_trips` |
| 完成后加入系统变量，不需要在代码中完成 | 程序**从不**修改环境变量；`install.ps1 -AddToPath` / `install.sh` 只做提示或经用户显式同意写入用户 PATH；README 第 3 节写了手工步骤 | 代码里没有 `setx`/`SetEnvironmentVariable` 调用；`Select-String -Pattern 'SetEnvironmentVariable' src/` 无命中 |

## 2. 关键设计决策

### 2.1 零依赖

构建机的 cargo registry 为空且无网络，因此 JSON（`src/json.rs`）与 UTC 历法（`src/time.rs`，
Hinnant civil_from_days 算法）都自己实现。副产品：`cargo build --offline` 永远可用，
发布产物只有一个 ~360 KB 的静态 exe。

### 2.2 为什么必须有 shell 包装函数

子进程改不了父 shell 的 cwd，这是操作系统层面的限制。`src/emit.rs` 提供两种交回目录的契约：

| 场景 | UI 输出 | 目录交回方式 |
| --- | --- | --- |
| `SCD_TARGET_FILE` 已设置（`shell/*` 使用） | stdout（像普通程序一样） | 写入该文件（UTF-8，无装饰） |
| 未设置 | stderr | stdout 只打印一行目录，`cd "$(scd …)"` 可直接用 |

选“写文件”而不是“读 stdout”的原因：Windows PowerShell 5.1 用控制台代码页解码原生命令的
stdout，中文目录会变问号；写文件 + `[IO.File]::ReadAllText(path, UTF8)` 则始终正确。
未安装函数时，程序会额外打印一条可直接粘贴的 `cd "..."`，并说明如何安装（`SCD_NO_HINT` 可关）。

### 2.3 lib + bin 双目标

`src/lib.rs` 暴露全部逻辑，`src/main.rs` 只有 8 行。集成测试既可以用
`env!("CARGO_BIN_EXE_scd")` 跑真进程（`tests/cli.rs`），也可以直接调用库函数做快速断言。

### 2.4 顺序固定，由用户控制（0.2.0 起）

0.1.0 曾按“最近使用”把位置移到首位，实际用下来问题更大：编号会漂移，
`scd 3` 今天和明天可能指向不同目录，肌肉记忆全部失效。0.2.0 改成：

* `Config::record_visit` 只累加 `visits` / 更新 `last_used_at`，**永不重排**；
  这两个字段退化为纯统计信息，只在 `scd list` 里展示。
* `Config::add` **追加到末尾**（返回新下标），已有编号不会因为新增而错位；
  若新下标 ≥ `max_display`，`add` 会额外提示「菜单看不到它，用 `[c]` 提高上限或 `scd mv n 1` 提前」。
* 排序是显式操作：`Config::move_to` / `Config::shift`（越界报错、到边夹紧，不环绕），
  对应 `scd mv <目标> <位置>`、`scd up/down <目标>`；改完立即打印新的顺序。
* 别名是显式操作：`Config::rename` 校验非空、≤60 字符、不与其他位置重名（重名会让
  `scd <名字>` 永久歧义），对应 `scd rename <目标> <新别名>`（名字可含空格）。
* `config.json` 里 `locations` 数组顺序 = 菜单顺序，手改即可，程序原样保留。

这样 `max_display` 的语义也更干净：显示的是**你排在前面的 n 个**，而不是“最近用的 n 个”。

### 2.5 原子写 + 不覆盖坏配置

`Config::save` 先写 `config.json.tmp` 再 `rename`（Windows 上 `rename` 会替换已存在文件），
中途崩溃最多留下一个 tmp。`Config::load` 遇到解析失败会带行列号报错并直接退出（码 2），
**绝不**用默认值覆盖用户文件，方便先手工修复。容忍 BOM、空文件、字符串简写、
`maxDisplay`/`display`/`dir`/`location` 等别名与未知键（向前兼容）。

### 2.6 目标解析顺序

`jump::resolve`：

1. 展开后确实存在的目录（`~`、相对路径、带引号、`.`/`..` 都支持）→ 直接跳
2. 展开后是文件 → 明确报“那是文件不是目录”
3. 纯数字 → 菜单编号（越界时再试一次名称匹配，兼容叫 `2024` 的目录）
4. 名称分层匹配：精确 → 前缀 → 包含 → **目录名分量前缀** → 模糊子序列
   （只比对路径的分量而不是整串，避免 `C:\Users\24263\...` 里的数字抢走短查询）
5. 多个命中 → 报歧义并列出 `[编号] 名称`

### 2.7 终端排版

所有用户可见文本统一 4 空格缩进（与需求文档里的示例一致）。
`ui::display_width` 按 UAX #11 把 CJK/全角/emoji 记为 2 列，
`pad_to_width` 据此补齐，所以 `docs` 与 `工作目录` 的路径能对齐在同一列。
读行用 `read_until` + `from_utf8_lossy` 并去掉前导 BOM：管道输入非法字节不会 panic，
PowerShell 管道常见的 BOM 也不会让第一次输入匹配不上 `[a]`/`[c]`。

## 3. 模块依赖

```text
main.rs ──> lib.rs::run
                 │
                 ├─ cli        解析 argv → Command / --help 文本
                 ├─ config     Config / Location / resolve_path / load / save
                 │    ├─ json  解析与序列化 config.json
                 │    ├─ paths 规范化、去重比较、默认名字
                 │    └─ time  added_at / last_used_at
                 ├─ menu       渲染菜单、主循环 ──> settings(c) / add(a) / jump
                 ├─ jump       resolve / perform ──> emit
                 ├─ emit       目录交回 shell（SCD_TARGET_FILE 或 stdout）
                 ├─ ui         缩进输出、读行、显示宽度、上色落笔
                 └─ color      ANSI 调色板、tty 检测、Windows VT 开关（0.2.0）
```

依赖是单向的：`ui`/`json`/`time`/`paths` 是叶子，`menu` 在顶层组合它们。

## 4. 控制流

**交互模式** `scd`：`cli::parse` → `config::resolve_path` → `Config::load` →
`menu::run`（渲染 → `scd> ` 读取 → 分派 `q`/`c`/`a`/其它当目标）→
`jump::perform`（校验是目录 → `touch` 记 visits 并前移 → `save` → `emit_jump`）→ 退出。

**直接模式** `scd <目标>`：同上但跳过菜单，直接 `jump::query`。
只读命令（`list`/`config`/`--help`/`--version`）永不写盘。

## 5. 扩展点

* **新增一个设置项**：在 `src/settings.rs` 的 `Setting` 加一个变体，实现
  `key/label/hint/current/apply`，再加进 `ALL`。菜单里的 `c` 流程会自动变成 `[1/2]`、`[2/2]`。
* **新增子命令**：在 `cli::Command` 加变体 + `cli::parse` 的关键字表 + `lib::run` 的分支。
* **换配置格式**：只需替换 `Config::from_json` / `to_json`，其余模块不感知 JSON。
* **位置排序策略**：改 `Config::move_to` / `shift` / `add`（例如提供 `scd sort visits` 让用户主动按访问次数排一次，或分组、frecency 打分）。

## 6. 测试策略

| 层级 | 位置 | 数量 | 覆盖 |
| --- | --- | --- | --- |
| 单元 | `src/*.rs` 里的 `#[cfg(test)] mod tests` | 110 | JSON 解析/序列化/错误定位、历法转换、路径规范化与同路径判定、配置读写与匹配分层、菜单排版（含 CJK 对齐）、设置项校验、目标解析各分支 |
| 端到端 | `tests/cli.rs` | 50 | 真跑 `scd` 二进制：菜单原文、`a`/`c` 流程、跳转（路径/编号/名称/相对/`~`/`--`）、MRU 与 visits、`max_display` 隐藏、中文与带空格路径、`SCD_TARGET_FILE` 契约、stdout 纯净性、损坏配置不被覆盖、退出码 0/1/2/64 |
| 安装 | `install.ps1` + `shell/scd.ps1` 实机跑 | 30 项断言 | `bin/`+`shell/`+`config.json` 结构、包装函数是否真的改变 `$PWD`、中文目录跳转、失败不乱跳、`list/config/help/version/add/rm/--config` 经函数转发后仍正确 |

每个端到端测试都用独立临时目录 + `SCD_CONFIG`，不会碰到开发者真实的配置文件。

## 7. 0.2.0：配色

新增 `src/color.rs`，仍然零依赖。三条硬规则：

1. **按流判定**：`Palette::for_stream(Stream)` 分别检测 stdout / stderr 是否为控制台
   （Windows 用 `GetConsoleMode`，Unix 用 `isatty`）。所以 `cd "$(scd proj)"` 里 stdout 是管道
   → 不上色，而 stderr 仍可以是彩色提示。
2. **机器可读通道永不上色**：`emit_jump` 的 stdout 目录行、`SCD_TARGET_FILE` 的内容、
   `config.json` 都绕过 palette。`tests/cli.rs` 用 `SCD_COLOR=always` 专门守这条契约。
3. **上色不改变文本**：`strip_ansi(coloured) == plain` 对菜单、列表、帮助、错误、
   添加/配置流程都有断言；`display_width` 先剥转义再计宽，所以对齐不受影响。

优先级：`SCD_COLOR=always|never` > `NO_COLOR` / `CLICOLOR=0` / `TERM=dumb` > tty 检测 >
`CLICOLOR_FORCE`。判定逻辑抽成纯函数 `decide()`，用真值表测试，不碰进程环境（避免并行测试互相干扰）。

角色配色：别名 `1;32`、路径 `1;36`、按键 `2`+`1;33`、参数 `33`、提示符 `1;35`、
成功 `1;32`、警告 `33`、错误 `1;31`、次要信息 `2`、帮助小节标题 `1;4`（粗体+下划线，
深浅背景都读得清，不像固定亮色会在白底上消失）。

Windows 上第一次决定要上色时会调用 `SetConsoleMode(..., ENABLE_VIRTUAL_TERMINAL_PROCESSING)`，
否则 cmd.exe / 老 conhost 会把 `[1;36m` 当普通文本打出来。FFI 直接 `unsafe extern "system"`
声明 kernel32 的三个函数，不引入 `winapi` / `windows-sys`。

## 8. 0.2.0：顺序与别名交给用户

需求（用户提出）：取消按使用次数排名、地址全部固定、别名可自己改。落点：

| 变化 | 代码 | 测试 |
| --- | --- | --- |
| 跳转不再重排 | `Config::record_visit`（原 `touch`） | `config::tests::record_visit_counts_without_reordering`、`jump::tests::perform_counts_the_visit_and_reports_the_path`、`tests/cli.rs::jumping_never_reorders_but_does_count_visits` |
| 新增追加到末尾 | `Config::add` 返回 `Added::New(len-1)` | `config::tests::add_appends_so_existing_numbers_never_shift`、`tests/cli.rs::add_appends_so_existing_numbers_stay_valid` |
| 手动排序 | `Config::move_to` / `shift` + `scd mv/up/down` | `config::tests::move_to_reorders_on_request`、`shift_moves_one_step_and_clamps`、`tests/cli.rs::mv_up_and_down_reorder_and_persist` |
| 改别名 | `Config::rename` + `scd rename` | `config::tests::rename_changes_the_alias_only`、`tests/cli.rs::rename_changes_the_alias_and_keeps_the_path`、`rename_refuses_empty_and_duplicate_names` |
| 手改顺序生效 | `Config::load` 原样保留数组顺序 | `tests/cli.rs::hand_edited_order_is_respected` |
| 新位置被隐藏时提示 | `add::interpret` 的 note 行 | `tests/cli.rs::a_new_entry_beyond_max_display_warns_and_stays_hidden` |

`scd mv/up/down/rename` 的 `<目标>` 复用 `jump::resolve`，所以编号、别名、路径三种写法都能指定；
执行成功后把新的完整顺序打印出来，用户不必再 `scd list` 确认。

## 9. 已知取舍

* **顺序不会自己变好**：常用目录不会自动上浮，需要用户 `scd mv/up` 或手改 `config.json`
  （这是 0.2.0 的有意取舍：编号稳定 > 自动优化）。
* **不做自动学习**：只有显式 `a` / `scd add` 才会入库，`scd <任意路径>` 不写历史，避免配置膨胀。
* **名称不是主键**：重名会自动加后缀（`same (nested)`、`same (2)`），匹配歧义时报错并列出候选。
* **cmd.exe 支持有限**：批处理包装要求 `<安装目录>\shell` 在 PATH 中排在 `bin` 之前，
  否则 `scd` 会命中 `scd.exe` 而无法切换目录（`PATHEXT` 顺序所致）。
* **非 UTF-8 路径**：写入 `config.json` 时用 `to_string_lossy`，极端情况下会丢字符；
  读入侧对非法字节同样容错，不会崩溃。
