# scd — super cd

在终端里快速跳转工作目录的小工具：敲 `scd` 弹出菜单，按数字就跳走；记得路径的话 `scd <路径>` 直接跳，连菜单都不用开。

* 用 Rust 编写，**零第三方依赖**（断网也能 `cargo build`）
* 配置就是一个 `config.json`，可以手改
* Windows / Linux / macOS 都支持；PowerShell、bash、zsh、fish、cmd.exe 都有配套函数
* 需求原文见 [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md)，实现说明见 [`docs/DESIGN.md`](docs/DESIGN.md)

---

## 1. 安装后的结构

`docs/ARCHITECTURE.md` 规定的结构，安装脚本会自动生成（位置由你自己决定）：

```
<你自己管理的位置>/scd
├─ bin/
│  └─ scd(.exe)      # 编译好的程序，这个目录要加入 PATH
├─ shell/
│  ├─ scd.ps1        # PowerShell 包装函数（真正执行 cd）
│  ├─ scd.bash       # bash
│  ├─ scd.zsh        # zsh
│  ├─ scd.fish       # fish
│  └─ scd.cmd        # cmd.exe
└─ config.json       # 你的配置：位置信息 + 显示多少个位置
```

## 2. 安装

### Windows（PowerShell）

```powershell
cd D:\data\Projects\scd
powershell -ExecutionPolicy Bypass -File .\install.ps1 -Dest D:\tools\scd -AddToPath -AddToProfile
```

| 参数 | 作用 |
| --- | --- |
| `-Dest <目录>` | 安装位置，默认 `$HOME\scd` |
| `-AddToPath` | 把 `<Dest>\bin` 追加到**用户** PATH（等价于你说的“加入系统变量”） |
| `-AddToProfile` | 往 `$PROFILE` 追加 `. "<Dest>\shell\scd.ps1"`，开新终端即可用 |
| `-SkipBuild` | 不重新编译，直接用现成的 `target\release\scd.exe` |
| `-Configuration debug` | 装 debug 版 |

不想让脚本碰环境变量就去掉 `-AddToPath`，脚本最后会打印手工步骤。

### Linux / macOS

```bash
./install.sh --dest ~/scd --add-to-rc     # 自动往 ~/.bashrc / ~/.zshrc / config.fish 追加 source
```

### 手工安装（任何平台）

```powershell
cargo build --release
mkdir D:\tools\scd\bin, D:\tools\scd\shell
copy target\release\scd.exe D:\tools\scd\bin\
copy shell\*               D:\tools\scd\shell\
```

`config.json` 不用自己建：第一次 `scd add` 会自动生成（内容见第 5 节）。

## 3. 加入系统变量（PATH）

程序自己不修改任何环境变量，这一步由你完成。

**图形界面**：`Win` → 搜索“编辑账户的环境变量” → 选中 `Path` → 新建 → 填入 `D:\tools\scd\bin` → 确定 → **重开终端**。

**命令行（用户级，无需管理员）**：

```powershell
[Environment]::SetEnvironmentVariable('Path', [Environment]::GetEnvironmentVariable('Path','User') + ';D:\tools\scd\bin', 'User')
```

**bash / zsh**：

```bash
echo 'export PATH="$HOME/scd/bin:$PATH"' >> ~/.bashrc && source ~/.bashrc
```

## 4. 让 `scd` 真的切换目录（重要）

操作系统规定：**子进程无法修改父 shell 的工作目录**。所以 `scd.exe` 负责“算出该去哪个目录”，真正的 `cd` 由一个同名 shell 函数完成。装一次即可：

| Shell | 一次性加载 | 永久生效 |
| --- | --- | --- |
| PowerShell | `. D:\tools\scd\shell\scd.ps1` | 把该行写进 `$PROFILE`（或安装时加 `-AddToProfile`） |
| bash | `source ~/scd/shell/scd.bash` | 写进 `~/.bashrc`（或 `--add-to-rc`） |
| zsh | `source ~/scd/shell/scd.zsh` | 写进 `~/.zshrc` |
| fish | `source ~/scd/shell/scd.fish` | 写进 `~/.config/fish/config.fish` |
| cmd.exe | 无需加载 | 把 `<安装目录>\shell` 放在 PATH 中 **`bin` 之前**（`scd.cmd` 会调用 `scd.exe`） |

如果 PowerShell 报“禁止运行脚本”，先执行一次：

```powershell
Set-ExecutionPolicy -Scope CurrentUser RemoteSigned
```

没装函数也能用：`scd` 会把算出的目录打印出来，并提示一条可直接粘贴的 `cd "..."`；也可以用 `cd "$(scd proj)"` 这种通用写法。

## 5. 使用

### 5.1 `scd` —— 打开菜单

```text
    [c] configure scd
    [a] add new location

    [1] scd     D:\data\Projects\scd
    [2] docs    D:\data\Projects\scd\docs
    [3] 工作目录  D:\工作目录

    [q] quit
    scd>
```

* 按 **数字** 跳到对应位置（跳转后该位置下次排到最前，最常用的一直在最上面）
* 按 **c** 进入配置：逐项询问需要修改的内容，目前只有一项——**最多显示多少个位置 n**；直接回车保持原值，输入 `quit` 取消。后续新增设置只需在 `src/settings.rs` 里加一项
* 按 **a** 添加位置，提示语与 `docs/ARCHITECTURE.md` 一致：

```text
    type the new location, or type "here" to add the current location
    type "quit" to cancel

    new location:
```

  输入绝对/相对路径、`~`、带引号的含空格路径、`here`（当前目录）都可以；`quit` 取消
* 按 **q** 退出；直接粘贴一个路径也能跳
* 菜单里也可以输入名称片段（如 `proj`）来跳转

### 5.2 `scd <目标>` —— 不开菜单直接跳

目标按以下顺序解析：

1. **目录路径**：`scd D:\data\Projects\scd`、`scd ~/code`、`scd ..\docs`、`scd .`、`scd "D:\my dir"`
2. **菜单编号**：`scd 3`
3. **已保存的名称**：`scd proj`（精确 → 前缀 → 包含 → 目录名 → 模糊匹配；有歧义会列出候选）

### 5.3 其它命令

```powershell
scd add .            # 保存当前目录（等价于菜单里的 a + here）
scd add "D:\my dir"  # 保存指定目录
scd rm proj          # 删除一个已保存位置（也可用编号或路径）
scd list             # 列出全部位置，包括被 n 限制隐藏的
scd config           # 显示当前使用的 config.json 路径
scd -h | scd --help  # 用户指南
scd -V | scd --version
scd --config D:\x\config.json list   # 临时用另一份配置
```

### 5.4 `config.json`

```json
{
  "version": 1,
  "max_display": 9,
  "locations": [
    {
      "name": "scd",
      "path": "D:\\data\\Projects\\scd",
      "visits": 3,
      "added_at": "2026-10-27T11:17:04Z",
      "last_used_at": "2026-10-27T11:17:04Z"
    }
  ]
}
```

| 字段 | 含义 |
| --- | --- |
| `max_display` | 菜单最多显示多少个位置（`c` 修改的就是它，范围 1–99） |
| `locations[].path` | 目录（也可只写一个字符串，例如 `"locations": ["D:\\code"]`） |
| `locations[].name` | 菜单里的名字，默认取文件夹名，可手改（重名会自动加后缀区分） |
| `visits` / `last_used_at` | 自动记录的访问次数与时间 |
| `added_at` | 添加时间 |

列表按“最近使用”排序，手改后保存即可生效；文件损坏时 scd 会报错并**拒绝覆盖**，方便你先备份。

查找顺序：`--config` → `SCD_CONFIG` → `SCD_HOME/config.json` → `<exe目录>/../config.json`（即 `bin/` 的上一级，符合规定结构）→ `<exe目录>/config.json` → `~/.scd/config.json`。

### 5.5 环境变量与退出码

| 变量 | 作用 |
| --- | --- |
| `SCD_CONFIG` | 指定配置文件 |
| `SCD_HOME` | 指定安装目录（含 `bin/` 与 `config.json`） |
| `SCD_BIN` | 让 shell 函数调用指定的可执行文件 |
| `SCD_TARGET_FILE` | 把选中目录写入该文件（包装函数使用） |
| `SCD_CD=1` | 声明调用方是 shell 函数，不再打印安装提示 |
| `SCD_NO_HINT=1` | 永不打印安装提示 |

退出码：`0` 成功（含退出菜单未跳转）· `1` 目标无法解析/不是目录 · `2` 配置文件读写失败 · `64` 命令行用法错误。

## 6. 开发

```powershell
cargo build            # 开发构建
cargo build --release  # 发布构建（target/release/scd.exe，约 360 KB）
cargo test             # 126 个测试：89 单元 + 37 端到端
cargo run -- --help
```

模块划分、设计取舍与需求对照见 [`docs/DESIGN.md`](docs/DESIGN.md)。

## 7. 排错

| 现象 | 原因 / 处理 |
| --- | --- |
| `scd` 打印了路径但目录没变 | 没加载 shell 函数，见第 4 节；或临时用 `cd "$(scd proj)"` |
| 提示 `scd: cannot find scd.exe` | `bin` 不在 PATH，或设置 `$env:SCD_BIN` 指向 exe |
| `scd` 被识别成 cmdlet/别名 | PowerShell 里函数未加载；`Get-Command scd` 检查来源 |
| 中文目录显示乱码 | 终端字体/代码页问题；建议 Windows Terminal + `chcp 65001`。`config.json` 始终是 UTF-8 |
| 修改 PATH 后不生效 | 需要**重开**终端窗口 |
| `scd: cannot parse ...config.json` | 手改时语法错了，按提示的行/列修正；scd 不会覆盖它 |
