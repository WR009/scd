# scd 实现说明

本文记录 `docs/ARCHITECTURE.md` 的每条要求落在哪里、为什么这么设计，以及怎样验证。
面向要改代码的人；使用者请看 [`README.md`](../README.md)。

## 1. 需求对照表

| 需求（ARCHITECTURE.md） | 实现 | 验证 |
| --- | --- | --- |
| 基于 Rust，终端里快速跳转工作目录 | 零依赖 Rust 二进制 `scd`（lib + bin） | `cargo test`：126 项 |
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

### 2.4 最近使用优先（MRU）

跳转/添加都会把该位置移到 `locations` 首位，菜单显示前 `max_display` 项。
这样“最多显示 n 个”才有意义：留在菜单里的一定是最常用的。
副作用是编号会变，所以 `scd <编号>` 只在刚看过菜单时可靠；记得路径就用路径或名称（文档本意如此）。

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
                 └─ ui         缩进输出、读行、显示宽度
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
* **位置排序策略**：改 `Config::touch` / `add`（例如按 visits 排序、分组、frecency 打分）。

## 6. 测试策略

| 层级 | 位置 | 数量 | 覆盖 |
| --- | --- | --- | --- |
| 单元 | `src/*.rs` 里的 `#[cfg(test)] mod tests` | 89 | JSON 解析/序列化/错误定位、历法转换、路径规范化与同路径判定、配置读写与匹配分层、菜单排版（含 CJK 对齐）、设置项校验、目标解析各分支 |
| 端到端 | `tests/cli.rs` | 37 | 真跑 `scd` 二进制：菜单原文、`a`/`c` 流程、跳转（路径/编号/名称/相对/`~`/`--`）、MRU 与 visits、`max_display` 隐藏、中文与带空格路径、`SCD_TARGET_FILE` 契约、stdout 纯净性、损坏配置不被覆盖、退出码 0/1/2/64 |
| 安装 | `install.ps1` + `shell/scd.ps1` 实机跑 | 30 项断言 | `bin/`+`shell/`+`config.json` 结构、包装函数是否真的改变 `$PWD`、中文目录跳转、失败不乱跳、`list/config/help/version/add/rm/--config` 经函数转发后仍正确 |

每个端到端测试都用独立临时目录 + `SCD_CONFIG`，不会碰到开发者真实的配置文件。

## 7. 已知取舍

* **编号会变**：MRU 排序导致 `scd 3` 的含义随使用变化（换来“常用的一直在菜单里”）。
* **不做自动学习**：只有显式 `a` / `scd add` 才会入库，`scd <任意路径>` 不写历史，避免配置膨胀。
* **名称不是主键**：重名会自动加后缀（`same (nested)`、`same (2)`），匹配歧义时报错并列出候选。
* **cmd.exe 支持有限**：批处理包装要求 `<安装目录>\shell` 在 PATH 中排在 `bin` 之前，
  否则 `scd` 会命中 `scd.exe` 而无法切换目录（`PATHEXT` 顺序所致）。
* **非 UTF-8 路径**：写入 `config.json` 时用 `to_string_lossy`，极端情况下会丢字符；
  读入侧对非法字节同样容错，不会崩溃。
