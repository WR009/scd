# Changelog

本文件记录 scd 的版本变化，版本号遵循语义化版本。

## 0.2.0

### 变更（行为）

* **取消按使用次数排名**：跳转不再把位置提到最前。`visits` / `last_used_at` 变成纯统计，
  只在 `scd list` 里显示。**顺序完全固定，由用户自己控制。**
* **新增位置追加到末尾**（原来是插到最前），已有编号不会再因为新增而错位。
  新位置落在 `max_display` 之外时会提示如何提高上限或把它提前。
* 菜单底部新增一行暗色提示，告诉你可以用哪些命令排序/改名。

### 新增（命令）

* `scd mv <目标> <位置>`：把某个位置放到第 n 位（1 起算，越界报错）。
* `scd up <目标>` / `scd down <目标>`：上移/下移一位，到顶到底自动停住（不环绕）。
* `scd rename <目标> <新别名>`：修改别名，新别名可含空格；空名、超长（>60）、
  与其他位置重名都会被拒绝并说明原因。
* 以上命令的 `<目标>` 支持编号、别名或路径三种写法；执行后打印新的完整顺序。
* `config.json` 里 `locations` 数组顺序即菜单顺序，手改存盘立即生效。

### 新增

* **全程配色**（`src/color.rs`，仍然零依赖）：
  * 位置别名粗体绿、路径粗体青 —— 这两类信息在菜单、列表和所有确认消息里最突出；
  * 按键 `[c] [a] [q] [1]` 用「暗色方括号 + 黄色字母」，命令行参数黄色，提示符品红，
    成功绿、警告黄、错误红，次要信息（`visits=`、`# config:`、页脚）暗色；
  * `scd --help` 有专门的排版器：小节标题粗体下划线，路径/文件名青色，环境变量黄色，
    `#` 注释暗色，并逐字保留原有列对齐；
  * Windows 上自动开启控制台 VT 处理，cmd.exe / 老 conhost 不会显示裸转义码；
  * 中日韩名字按 2 列计宽、转义序列不计宽，混排仍然对齐。
* **发布工程**：`tools/dist.ps1` / `tools/dist.sh` 一键打包，产物命名用 Rust target triple
  （`scd-0.2.0-x86_64-pc-windows-msvc.zip`）并附 `.sha256`；包内就是文档规定的
  `bin/ + shell/ + config.example.json` 结构，解压即用。
* `install.cmd`：Windows 双击安装（自动 `-ExecutionPolicy Bypass`，并加 PATH 与 `$PROFILE`）。
* `install.ps1` / `install.sh` 自动识别"源码仓库"与"解压出来的发布包"：后者不需要装 Rust。
* `install.ps1 -AddToPath` 现在会把 `<安装目录>\shell` 一起加进用户 PATH，并排在 `bin` **之前**：
  cmd.exe 没有函数机制，只有 `shell\scd.cmd` 排前面才能真正切换目录。
* `install.sh --add-to-rc` 除了 `source` 行，还会写入 `export PATH="<dest>/bin:$PATH"`，
  一条命令装完，新开终端直接可用。
* `install.ps1` 检测到 ExecutionPolicy 为 Restricted 时会明确提示：`$PROFILE` 不会运行、
  函数不会加载，需要执行一次 `Set-ExecutionPolicy -Scope CurrentUser RemoteSigned`。
* README 重写为两部分：前半段纯用户视角（下载 release 包 → 双击安装 → 日常使用 → 排错），
  后半段才是从源码构建与发布流程。
* `.github/workflows/release.yml`：推 `v*.*.*` 标签时在 windows / ubuntu(gnu+musl) / macos
  原生 runner 上测试、编译、打包并自动挂到 GitHub Release。
* 新增 `LICENSE`（MIT，此前 Cargo.toml 已声明但缺文件）、`config.example.json`，
  `.gitignore` 忽略 `releases/`。
* 配色开关：`SCD_COLOR=auto|always|never`，并遵守 `NO_COLOR`、`CLICOLOR=0`、
  `CLICOLOR_FORCE`、`TERM=dumb` 约定；默认 auto，输出被重定向时自动关闭。
* `color::strip_ansi` 与「上色不改变文本」的回归测试；`target/preview.html` 预览页。

### 保证不变

* 机器可读通道**永不上色**：非包装模式下 stdout 的目录行、`SCD_TARGET_FILE` 的内容、
  `config.json`。`cd "$(scd proj)"` 与所有 shell 包装函数的行为与 0.1.0 一致。
* `SCD_COLOR=never`（以及任何重定向场景）下的输出与 0.1.0 逐字节相同。

### 测试

* 160 项（110 单元 + 50 端到端），新增配色契约、帮助排版不变式、NO_COLOR 优先级，
  以及顺序固定、`mv`/`up`/`down`/`rename`、手改顺序、隐藏提示等用例。

## 0.1.0

* 首个可用版本：`scd` 交互菜单（`[c]` 配置 / `[a]` 添加 / `[1..n]` 跳转 / `[q]` 退出）、
  `scd <目标>` 直接跳转（路径 / 编号 / 名称）、`add` `rm` `list` `config` 子命令、
  `--help` 用户指南、`config.json` 原子读写、MRU 排序、`max_display` 限制、
  PowerShell / bash / zsh / fish / cmd.exe 包装函数，以及 `install.ps1`、`install.sh`。
