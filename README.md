# scd — super cd

在终端里快速跳转工作目录的小工具：敲 `scd` 弹出菜单，按数字就跳走；记得路径的话 `scd <路径>` 直接跳，连菜单都不用开。

* 用 Rust 编写，**零第三方依赖**（断网也能 `cargo build`）
* **0.2.0 起全程配色**：别名绿色、路径青色、按键黄色、错误红色，重定向时自动关闭
* **0.2.0 起顺序固定**：不再按使用次数排名，新位置追加到末尾，别名可用 `scd rename` 改，顺序可用 `scd mv/up/down` 调
* 配置就是一个 `config.json`，可以手改
* Windows / Linux / macOS 都支持；PowerShell、bash、zsh、fish、cmd.exe 都有配套函数
* 需求原文见 [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md)，实现说明见 [`docs/DESIGN.md`](docs/DESIGN.md)

---

## 0. 从 GitHub Release 下载后怎么用（即开即用）

1. 解压 `scd-<版本>-x86_64-pc-windows-msvc.zip` 到任意位置（里面已经是一个可用的安装目录）。
2. **双击 `install.cmd`**
   —— 它会用 `-ExecutionPolicy Bypass` 调 `install.ps1 -AddToPath -AddToProfile`，
   把程序装到 `%USERPROFILE%\scd`、生成 `config.json`、把 `bin` 加进用户 PATH、把 shell 函数写进 `$PROFILE`。
   想装到别处：`install.cmd -Dest D:\tools\scd`。不想让它碰环境变量：手动跑
   `powershell -ExecutionPolicy Bypass -File .\install.ps1 -Dest D:\tools\scd`（脚本会打印手工步骤）。
3. **开一个新终端**，敲 `scd`。

> 不想安装也能用：解压后直接跑 `bin\scd.exe`，配置会落到 `~\.scd\config.json`；
> 但要让 `scd` 真的切换目录，仍然需要第 2 步（子进程改不了父 shell 的工作目录，见第 4 节）。

Linux / macOS：解压 `*.tar.gz` 后执行 `./install.sh --dest ~/scd --add-to-rc`（发布包里已带编译好的 `bin/scd`，无需 Rust）。

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
├─ config.json       # 你的配置：位置信息 + 显示多少个位置（首次运行时生成）
├─ config.example.json  # 模板，发布包里附带的是这一份
├─ install.cmd       # Windows 双击安装（Release 包里就有）
├─ install.ps1 / install.sh
└─ README.md / CHANGELOG.md / LICENSE
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

* 按 **数字** 跳到对应位置。**顺序是固定的**：跳转不会重排，编号永远指同一个位置
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
scd rename 2 项目     # 改别名（支持空格：scd rename 2 my project）
scd mv proj 1        # 把某个位置放到第 1 位
scd up proj          # 上移一位（scd down proj 下移一位，到顶/到底会自动停住）
scd list             # 列出全部位置，包括被 n 限制隐藏的
scd config           # 显示当前使用的 config.json 路径
scd -h | scd --help  # 用户指南
scd -V | scd --version
scd --config D:\x\config.json list   # 临时用另一份配置
```

### 5.4 顺序与别名都由你控制

* **不按使用次数排名**：`scd` 从不自动重排。跳转只会累加 `visits` / 更新 `last_used_at`
  这两个统计值（`scd list` 里能看到），**不影响顺序**。
* **新增位置追加到末尾**，所以你已经记住的编号不会因为加了一个新目录而错位。
  如果新位置落在 `max_display` 之外，`scd add` 会直接提示你怎么把它提上来。
* **想自己排**：`scd mv <目标> <位置>`、`scd up/down <目标>`，或者直接编辑 `config.json`
  —— 数组顺序就是菜单顺序，改完存盘立即生效。
* **想改别名**：`scd rename <目标> <新别名>`（目标可以是编号、旧别名或路径；新别名可含空格）。
  重名会被拒绝并告诉你是谁占了这个名字，也可以直接改 `config.json` 里的 `name`。
* 每次改动后 `scd` 会把新的顺序打印出来，不用再敲一次 `scd list` 确认。

### 5.5 `config.json`

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
| `visits` / `last_used_at` | 自动记录的访问次数与时间（**只是统计，不参与排序**） |
| `added_at` | 添加时间 |

**数组顺序就是菜单顺序**，手改后保存即可生效；文件损坏时 scd 会报错并**拒绝覆盖**，方便你先备份。

查找顺序：`--config` → `SCD_CONFIG` → `SCD_HOME/config.json` → `<exe目录>/../config.json`（即 `bin/` 的上一级，符合规定结构）→ `<exe目录>/config.json` → `~/.scd/config.json`。

### 5.6 配色（0.2.0）

在真实终端里 `scd` 的所有输出都会上色，角色固定：

| 元素 | 颜色 | 例子 |
| --- | --- | --- |
| **位置别名 / 文件夹名** | 粗体绿 | `scd`、`工作目录` |
| **路径** | 粗体青 | `D:\data\Projects\scd` |
| 可按键位 | 黄色字母 + 暗色方括号 | `[c]` `[a]` `[1]` `[q]` |
| 命令行参数 | 黄色 | `--help`、`--config` |
| 提示符 | 粗体品红 | `scd>`、`new location:` |
| 成功 / 警告 / 错误 | 粗体绿 / 黄 / 粗体红 | `added`、`hint:`、`scd:` |
| 次要信息 | 暗色 | `[q] quit`、`visits=3`、`# config:` |
| 帮助里的小节标题 | 粗体 + 下划线 | `USAGE:`（不依赖亮色，深浅背景都清楚） |

规则：

* **默认 auto**：只有输出真的连到终端时才上色；一旦重定向/管道（例如 `cd "$(scd proj)"`、`scd list > a.txt`）自动变成纯文本。
* **机器可读通道永不上色**，无论怎么设置：非包装模式下的 stdout 目录行、`SCD_TARGET_FILE` 里的内容、`config.json`。这是 `cd` 能安全套用的前提，已有专门测试守着。
* Windows 上程序会自己打开控制台的 VT 处理（`ENABLE_VIRTUAL_TERMINAL_PROCESSING`），所以老 conhost / cmd.exe 里也不会看到 `[1;36m` 这种乱码。
* 中日韩名字按 **2 列**计宽（`display_width`），转义序列不计宽，所以 `docs` 和 `工作目录` 的路径仍然对齐在同一列。

开关：

```powershell
$env:SCD_COLOR = 'always'   # 强制上色（演示、截图、写测试）
$env:SCD_COLOR = 'never'    # 强制关闭
$env:NO_COLOR  = '1'        # 等价于 never（no-color.org 约定）
$env:CLICOLOR_FORCE = '1'   # 即使重定向也上色
```

想先看效果：`cargo build --release` 后运行 `powershell -File tools\preview.ps1`，再打开生成的 `target\preview.html` —— 里面是真实二进制输出转成的深/浅两种背景预览。

### 5.7 环境变量与退出码

| 变量 | 作用 |
| --- | --- |
| `SCD_CONFIG` | 指定配置文件 |
| `SCD_HOME` | 指定安装目录（含 `bin/` 与 `config.json`） |
| `SCD_BIN` | 让 shell 函数调用指定的可执行文件 |
| `SCD_TARGET_FILE` | 把选中目录写入该文件（包装函数使用） |
| `SCD_CD=1` | 声明调用方是 shell 函数，不再打印安装提示 |
| `SCD_NO_HINT=1` | 永不打印安装提示 |
| `SCD_COLOR=auto\|always\|never` | 配色开关，默认 auto（只在终端上色） |
| `NO_COLOR=1` | 等价于 `SCD_COLOR=never` |
| `CLICOLOR_FORCE=1` | 重定向时也上色 |

退出码：`0` 成功（含退出菜单未跳转）· `1` 目标无法解析/不是目录 · `2` 配置文件读写失败 · `64` 命令行用法错误。

## 6. 开发

```powershell
cargo build            # 开发构建
cargo build --release  # 发布构建（target/release/scd.exe，约 360 KB）
cargo test             # 160 个测试：110 单元 + 50 端到端
cargo run -- --help
```

模块划分、设计取舍与需求对照见 [`docs/DESIGN.md`](docs/DESIGN.md)。

### 打包与发布

本机打包（产物落在 `releases/`，已被 `.gitignore` 忽略）：

```powershell
.\tools\dist.ps1                                   # Windows：releases\Windows_x64\ + zip + .sha256
.\tools\dist.ps1 -SkipBuild                        # 复用已有构建
```

```bash
./tools/dist.sh --target x86_64-unknown-linux-gnu   --label Linux_x64        # 在 Linux 上打包
./tools/dist.sh --target x86_64-unknown-linux-musl  --label Linux_x64_static # 静态版，任何发行版都能跑
./tools/dist.sh --target aarch64-apple-darwin       --label macOS_arm64
```

正式发版走 CI，不要在自己电脑上凑其它平台的产物：

```bash
# 1) 改 Cargo.toml 的 version + 写 CHANGELOG.md
# 2) 提交并打标签
git tag v0.2.0 && git push origin v0.2.0
```

`.github/workflows/release.yml` 会在 windows / ubuntu / macos runner 上跑测试、编译、
用同一个 `tools/dist.sh` 打包，并把 `zip` / `tar.gz` / `.sha256` 自动挂到 GitHub Release。
上传件命名遵循 Rust target triple，例如 `scd-0.2.0-x86_64-pc-windows-msvc.zip`。

## 7. 排错

| 现象 | 原因 / 处理 |
| --- | --- |
| `scd` 打印了路径但目录没变 | 没加载 shell 函数，见第 4 节；或临时用 `cd "$(scd proj)"` |
| 提示 `scd: cannot find scd.exe` | `bin` 不在 PATH，或设置 `$env:SCD_BIN` 指向 exe |
| `scd` 被识别成 cmdlet/别名 | PowerShell 里函数未加载；`Get-Command scd` 检查来源 |
| 中文目录显示乱码 | 终端字体/代码页问题；建议 Windows Terminal + `chcp 65001`。`config.json` 始终是 UTF-8 |
| 修改 PATH 后不生效 | 需要**重开**终端窗口 |
| `scd: cannot parse ...config.json` | 手改时语法错了，按提示的行/列修正；scd 不会覆盖它 |
