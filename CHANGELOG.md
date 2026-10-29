# Changelog

本文件记录 scd 的版本变化，版本号遵循语义化版本。

## 0.2.2

### 修复

* **装到没有写权限的目录不再留下半成品**：`install.ps1` 现在在动任何文件之前先探测目标是否可写
  （目录本身、`bin\`、`shell\`，以及已有 `scd.exe` 能否以写方式独占打开）。
  `D:\applications` 这类目录通常继承 `Everyone:(RX)`，只有管理员能写；0.2.1 及以前会在复制到一半时
  抛 `Copy-Item : Access to the path ... is denied`，结果是**文档已经换成新版、exe 还是旧版**，
  只报一个 exit 1，用户看不出装成了没有。现在会立刻以 **exit code 3** 退出、一个文件都不改，
  并印出三条出路：装到自己有权限的目录 / 右键以管理员身份重跑 / 用 `icacls` 给自己授权
  （命令按你的目录和用户拼好，直接复制粘贴即可）。
* `install.cmd` 在失败时解释 exit code 3 的含义，并提示可以右键"以管理员身份运行"。

## 0.2.1

### 变更（发布范围）

* **只发布 Windows x64**：CI 不再为 Linux（gnu / musl）和 macOS 构建产物。Windows x64 是唯一
  由作者本人在实机上完整走通安装流程的平台，发布没人验证过的二进制比不发布更糟。
  `install.sh`、`tools/dist.sh`、`shell/scd.{bash,zsh,fish}` 仍保留在仓库里供自行构建。
* **README 提供中英两版**：`README.md`（中文）与 `README.en.md`（English）互相链接，
  两份都在开头写明「目前只有 Windows x64 经过人工验证」。两个文件都随包发布。

### 修复

* **发布包里的 POSIX 脚本不再是 CRLF**：CI 的 Windows runner 以 `core.autocrlf=true` 检出，
  0.2.0 的 zip 里 `install.sh`、`scd.bash`、`scd.zsh`、`scd.fish` 全都带 CR，在 Git Bash 下
  会 `$'\r': command not found`。新增 `.gitattributes` 把这些扩展名钉死为 `eol=lf`
  （`.cmd` / `.bat` 钉为 `eol=crlf`，`*.exe`、压缩包标记为 binary），并且 `tools/dist.ps1`
  在打包时扫到任何 CR 就直接失败 —— 坏包发不出去。
* `tools/dist.ps1`：`$isWindows` 改名 `$forWindows`。PowerShell 7（CI 用的 `shell: pwsh`）里
  `$IsWindows` 是只读自动变量，给它赋值会让脚本在第一行就中止，Windows job 因此只报一个
  裸的 exit code 1；本地 Windows PowerShell 5.1 没有这个变量，所以一直没暴露。

### 新增

* `install.cmd` 双击安装时**先问装到哪个目录**（回车用默认 `%USERPROFILE%\scd`，也可以输入
  `D:\applications\scd` 这类路径，带引号的空格路径会被正确处理）；带参数运行则跳过询问，
  例如 `install.cmd -Dest D:\applications\scd`。
* `tools/dist.ps1` 打包时带上 `README.en.md`。

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
* 端到端沙箱的根目录改为规范化路径（`paths::absolute`）：CI runner 的临时目录在 Windows 上是
  8.3 短名（`C:\Users\RUNNER~1\...`）、在 macOS 上要经过 `/var -> /private/var` 符号链接，
  拼写与 scd 报告的规范化路径不同，导致 `add` 的两个断言只在 CI 上失败、本地全绿。
* 给子进程写 stdin 时容忍 `BrokenPipe`：被测程序本来就可能读完输入之前就退出（配置损坏、
  `quit` 等），静态 musl 构建跑得快时会赢下这个竞争，测试因此偶发失败。
* `.github/workflows/release.yml`：`cargo test` 失败时把关键行发成 check-run annotation ——
  job 日志只有仓库管理员能下载，annotation 走公开 API 就能读到。

## 0.1.0

* 首个可用版本：`scd` 交互菜单（`[c]` 配置 / `[a]` 添加 / `[1..n]` 跳转 / `[q]` 退出）、
  `scd <目标>` 直接跳转（路径 / 编号 / 名称）、`add` `rm` `list` `config` 子命令、
  `--help` 用户指南、`config.json` 原子读写、MRU 排序、`max_display` 限制、
  PowerShell / bash / zsh / fish / cmd.exe 包装函数，以及 `install.ps1`、`install.sh`。
