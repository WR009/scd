# scd — super cd

终端里的“目录收藏夹”：敲 `scd`，按个数字，立刻跳到常去的目录，不用再打一长串路径。

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

- **不用装环境**：一个 exe，零依赖。不需要 Rust、不需要 Python、不需要管理员权限
- **编号永远稳定**：顺序和别名完全由你控制，scd 绝不会按使用频率偷偷重排
- **全 shell 通用**：PowerShell / cmd.exe / bash / zsh / fish
- **配置就是 JSON**：`config.json` 可以直接用记事本改

---

# 第一部分 · 用户指南（下载即用）

## 1. 下载

打开 **[Releases 页面](https://github.com/WR009/scd/releases)**，按你的系统下载一个包：

| 系统 | 文件 | 备注 |
| --- | --- | --- |
| Windows 10 / 11 (x64) | `scd-0.2.0-x86_64-pc-windows-msvc.zip` | 解压 → 双击 `install.cmd` |
| Linux (x64) | `scd-0.2.0-x86_64-unknown-linux-gnu.tar.gz` | Ubuntu / Debian / Fedora / Arch |
| Linux (x64，静态) | `scd-0.2.0-x86_64-unknown-linux-musl.tar.gz` | 任何发行版、任何容器都能跑 |
| macOS (Apple Silicon) | `scd-0.2.0-aarch64-apple-darwin.tar.gz` | M1 / M2 / M3 / M4 |

每个包旁边都有同名的 `.sha256` 校验文件，需要时可自行核对。当前版本 **0.2.0**，改动见 [CHANGELOG.md](CHANGELOG.md)。

## 2. Windows 安装（三步）

1. **下载**上面的 `.zip`
2. **解压**到任意位置（右键 → 全部解压）
3. **双击解压目录里的 `install.cmd`**

看到 `Done. Open a NEW terminal and type: scd` 就成功了。**开一个新终端**，敲 `scd` 即可。

`install.cmd` 会把下面这些事全做完，你不需要手动配任何环境变量：

| 它做了什么 | 具体内容 |
| --- | --- |
| 安装程序 | 复制到 `%USERPROFILE%\scd\bin\scd.exe` |
| 加入 PATH | 把 `...\scd\bin` 和 `...\scd\shell` 写进**用户** PATH（无需管理员） |
| 让 `scd` 真能切目录 | 往 PowerShell 的 `$PROFILE` 追加一行 `. "...\scd\shell\scd.ps1"` |

> **想装到别的目录**（例如 `D:\applications\scd`）：
>
> ```
> install.cmd -Dest D:\applications\scd
> ```
>
> 或者在 PowerShell 里：
>
> ```powershell
> powershell -NoProfile -ExecutionPolicy Bypass -File .\install.ps1 -Dest D:\applications\scd -AddToPath -AddToProfile
> ```

**安装后的目录：**

```
%USERPROFILE%\scd\
├── bin\scd.exe          程序本体
├── shell\               各 shell 的 scd 函数（scd.ps1 / scd.cmd / scd.bash / scd.zsh / scd.fish）
├── config.json          你保存的目录（可用记事本直接改）
├── config.example.json  配置示例
├── README.md  LICENSE  CHANGELOG.md
└── install.cmd  install.ps1  install.sh
```

**升级**：解压新版 zip，再双击一次 `install.cmd`。你的 `config.json` 会原样保留。

## 3. Linux / macOS 安装

```bash
tar -xzf scd-0.2.0-x86_64-unknown-linux-gnu.tar.gz
cd scd-0.2.0-x86_64-unknown-linux-gnu
bash install.sh --add-to-rc                      # 默认装到 ~/scd
bash install.sh --dest ~/.local/scd --add-to-rc  # 或指定目录
```

`--add-to-rc` 会往 `~/.bashrc` / `~/.zshrc` / `~/.config/fish/config.fish` 追加两行（PATH + source）。**开一个新终端**即可使用。

## 4. 日常使用

### 打开菜单：`scd`

| 按键 | 作用 |
| --- | --- |
| `1` `2` `3` … | 跳到对应目录 |
| `a` | 添加新位置（输入路径，或输入 `here` 表示当前目录，`quit` 取消） |
| `c` | 设置（菜单最多列几条等） |
| `q` | 退出 |
| 其它任何输入 | 当成目标解析，所以也可以直接粘贴一条路径 |

### 不开菜单直接跳：`scd <目标>`

目标按这个顺序尝试：

1. **目录路径** —— `scd D:\code`、`scd ~/code`、`scd ..\docs`、`scd "C:\my dir"`、`scd .`
2. **菜单编号** —— `scd 3`（`scd list` 里看到的那个号）
3. **位置名称** —— `scd proj`（支持精确 / 前缀 / 子串 / 路径 / 模糊匹配）

### 管理保存的位置

| 命令 | 作用 |
| --- | --- |
| `scd add .` | 把当前目录加入配置（**追加到末尾**，老编号不变） |
| `scd add "D:\my dir"` | 加入指定目录 |
| `scd rm <目标>` | 删除 |
| `scd rename <目标> <新别名>` | 改别名，可以带空格和中文 |
| `scd mv <目标> <n>` | 把它放到第 n 位 |
| `scd up <目标>` / `scd down <目标>` | 上移 / 下移一位 |
| `scd list` | 列出全部（含访问次数等统计） |
| `scd config` | 显示当前用的是哪个 `config.json` |
| `scd --help` | 完整使用说明 |

## 5. 顺序和别名都由你控制

scd **不会**按使用频率自动重排：

- 新位置一律**追加到末尾**，你已经记住的编号不会失效
- 想调顺序用 `scd mv / up / down`，或者直接用记事本改 `config.json` 里 `locations` 数组的顺序（**数组顺序 = 菜单顺序**）
- `visits`、`last_used_at` 只是统计数字，`scd list` 会显示，但**不参与排序**

## 6. 配置文件

默认在 `<安装目录>\config.json`（Windows）或 `~/scd/config.json`（Linux / macOS），可以手改。scd 每次写入都是原子的（先写临时文件再替换），不会写坏文件。

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

`max_display` 就是菜单里 `[c] configure scd` 能改的那一项。

> 查找顺序：`--config` → `$SCD_CONFIG` → `$SCD_HOME\config.json` → exe 位于 `bin\` 时的上一级目录 → exe 同级目录 → `~/.scd/config.json`

## 7. 配色

真实终端里自动上色：别名绿色、路径青色、按键提示黄色、错误红色、次要信息灰色；管道和重定向时自动关闭，脚本里拿到的永远是干净文本。

| 想做什么 | 怎么做 |
| --- | --- |
| 强制开 / 关 | `SCD_COLOR=always` / `SCD_COLOR=never` |
| 遵循通用约定 | 设置 `NO_COLOR=1` |

## 8. 卸载

1. 删掉安装目录（默认 `%USERPROFILE%\scd`）
2. 从用户 PATH 里删掉 `...\scd\bin` 和 `...\scd\shell` 两条
3. 从 `$PROFILE`（或 `~/.bashrc`）里删掉那行 `. "...\shell\scd.ps1"` / `source "..."`

## 9. 排错

| 现象 | 原因与解决 |
| --- | --- |
| `'scd' 不是内部或外部命令` / `command not found` | PATH 是安装时才改的，**必须开新终端**；或检查用户 PATH 里有没有 `...\scd\bin` |
| 显示了目录但没跳过去，还提示 `scd cannot change this shell's directory by itself` | shell 函数没加载。PowerShell：`$PROFILE` 里要有 `. "<安装目录>\shell\scd.ps1"`（`install.cmd` 已自动写）；bash/zsh：rc 文件里要有对应的 `source` 行 |
| PowerShell 报“因为在此系统上禁止运行脚本” | 执行策略是 Restricted，`$PROFILE` 根本不会运行。执行一次即可（只影响当前用户，不需要管理员）：`Set-ExecutionPolicy -Scope CurrentUser RemoteSigned` |
| cmd.exe 里能显示目录但不切过去 | `shell` 目录必须排在 `bin` **前面**（cmd.exe 在同目录里优先选 `.exe`）。用 `install.cmd` 安装会自动排好；手动加请注意顺序 |
| 只想临时用一下，不想装 shell 函数 | `cd "$(scd proj)"`（PowerShell / bash 都可用） |
| 不想要颜色 | `SCD_COLOR=never` 或 `NO_COLOR=1` |
| 双击 `install.cmd` 失败 | 用 PowerShell 手动跑 `install.ps1` 看完整报错；或确认安装目录有写权限 |

---

# 第二部分 · 从源码构建（开发者）

> 只是用一下的话不需要看这部分：Release 里已经有编译好的二进制，装 Rust 只为了改代码。

## 1. 环境

- Rust **stable**（`edition 2024`，即 1.85 或更新），用 [rustup](https://rustup.rs) 安装
- **零第三方依赖**：`Cargo.toml` 的 `[dependencies]` 是空的。JSON 解析、UTC 时间、ANSI 配色全部手写，因此从干净检出**离线也能构建**，CI 里的 `--locked` 不会去下载任何东西

## 2. 构建与测试

```bash
cargo build --release   # -> target/release/scd(.exe)
cargo test              # 单元测试 + tests/cli.rs 里的端到端测试
cargo clippy --all-targets   # 可选
```

## 3. 从仓库安装

仓库里的 `install.ps1` / `install.sh` 会自己调 cargo 构建再安装。它们和 Release 包里是**同一个脚本**：脚本会判断自己在源码仓库里（有 `Cargo.toml`，需要构建）还是在解压出来的包里（有 `bin/scd`，直接装）。

```powershell
.\install.ps1 -Dest D:\applications\scd -AddToPath -AddToProfile
```

```bash
bash install.sh --dest ~/.local/scd --add-to-rc
```

## 4. 代码结构

```
src/
├── main.rs      入口
├── cli.rs       参数解析与子命令分发
├── lib.rs       库入口（供测试使用）
├── config.rs    config.json 读写：原子替换 + 结构校验
├── json.rs      手写的极简 JSON 解析 / 序列化
├── time.rs      手写的 UTC 时间（无 chrono）
├── color.rs     ANSI 配色、tty 探测、Windows VT 启用、配色策略
├── paths.rs     配置文件查找顺序、路径规范化
├── menu.rs      交互式菜单
├── jump.rs      目标解析：路径 / 编号 / 名称（五级匹配）
├── add.rs       add / rm / mv / up / down / rename
├── emit.rs      输出通道：人类可读 vs 机器可读（后者永不上色）
├── settings.rs  `[c] configure` 里的设置项
└── ui.rs        终端渲染
shell/           五种 shell 的 scd 函数 / 包装
tools/dist.ps1   打包脚本（Windows）
tools/dist.sh    打包脚本（Linux / macOS）
tools/preview.ps1  生成配色预览 target/preview.html
tests/cli.rs     端到端测试
docs/            ARCHITECTURE.md（规格）、DESIGN.md（设计决策）
```

## 5. 打包与发布

发布**不需要**在本地交叉编译：`.github/workflows/release.yml` 会在原生 runner 上把四个平台的包全部构建好并挂到 GitHub Release（产物就是本文开头那张表）。

```bash
# 1) 改 Cargo.toml 里的 version，写 CHANGELOG.md
# 2) 提交并打标签
git tag -a v0.3.0 -m "scd 0.3.0"
git push origin master --tags
```

标签一推上去，CI 自动执行：`cargo test --locked` → `cargo build --release --locked` → 调 `tools/dist.ps1|dist.sh` 打包 → 冒烟测试包里的二进制 → 上传到 Release。矩阵是 `fail-fast: false`，单个平台失败不影响其它平台出包。

本地只想出一个包（不打标签、不发布）：

```powershell
.\tools\dist.ps1 -Target x86_64-pc-windows-msvc -Label Windows_x64 -Ext zip
```

```bash
./tools/dist.sh --target x86_64-unknown-linux-gnu --label Linux_x64 --ext tar.gz
```

产物落在 `releases/`（已 gitignore，永不入库）：`scd-<版本>-<target>.<ext>` + `.sha256`，外加一个解开好的 `releases/<Label>/`，方便直接测试安装流程。

## 6. 完整参考

**环境变量**

| 变量 | 作用 |
| --- | --- |
| `SCD_CONFIG=<file>` | 指定配置文件 |
| `SCD_HOME=<dir>` | 指定含 `bin/` 和 `config.json` 的安装目录 |
| `SCD_BIN=<exe>` | shell 函数应当调用的二进制 |
| `SCD_TARGET_FILE=<file>` | 把选中的目录写到该文件（shell 包装就是这样拿到结果的） |
| `SCD_CD=1` | 调用方是 shell 函数：不再打印安装提示 |
| `SCD_NO_HINT=1` | 永不打印安装提示 |
| `SCD_COLOR=auto\|always\|never` | 配色策略，默认 `auto`（只在真终端上色） |
| `NO_COLOR=1` | 等价于 `SCD_COLOR=never` |
| `CLICOLOR_FORCE=1` | 即使输出被重定向也上色 |

**退出码**

| 码 | 含义 |
| --- | --- |
| 0 | 成功（在菜单里直接退出也算成功） |
| 1 | 目标解析失败，或它不是一个目录 |
| 2 | `config.json` 读 / 解析 / 写失败 |
| 64 | 命令行本身写错了 |

**为什么必须有 shell 函数？** 任何子进程都无法修改父 shell 的工作目录。所以 `scd` 负责解析出目标目录并回传，真正的 `cd` 由 `shell/` 里的同名函数完成。装上之后，`scd` 用起来就和 `cd` 一模一样。

## 7. 更多文档

- [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) —— 功能规格
- [docs/DESIGN.md](docs/DESIGN.md) —— 设计决策（为什么零依赖、为什么不做使用频率重排、配色策略等）
- [CHANGELOG.md](CHANGELOG.md) —— 版本历史

## License

MIT © 2026 River
