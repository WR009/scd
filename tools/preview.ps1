$ErrorActionPreference = 'Continue'
[Console]::OutputEncoding = [Text.Encoding]::UTF8
$repo = Split-Path -Parent $PSScriptRoot
$exe  = "$repo\target\release\scd.exe"
$work = "$repo\target\preview"
if (Test-Path -LiteralPath $work) { Remove-Item -LiteralPath $work -Recurse -Force }
foreach ($d in 'scd','docs','工作目录','my project','a-very-long-folder-name') {
    New-Item -ItemType Directory -Force -Path (Join-Path $work $d) | Out-Null
}
$cfgDir = Join-Path $work 'cfg'
New-Item -ItemType Directory -Force -Path $cfgDir | Out-Null
$cfg = Join-Path $cfgDir 'config.json'

$names = @('scd','docs','工作目录','my project','long')
$dirs  = @('scd','docs','工作目录','my project','a-very-long-folder-name')
$parts = @()
for ($i = 0; $i -lt $names.Count; $i++) {
    $esc = (Join-Path $work $dirs[$i]).Replace('\','\\')
    $visits = 7 - $i
    $parts += "    { `"name`": `"$($names[$i])`", `"path`": `"$esc`", `"visits`": $visits, `"added_at`": `"2026-10-20T01:00:00Z`", `"last_used_at`": `"2026-10-28T09:30:00Z`" }"
}
$json = "{`n  `"version`": 1,`n  `"max_display`": 3,`n  `"locations`": [`n" + ($parts -join ",`n") + "`n  ]`n}`n"
[IO.File]::WriteAllText($cfg, $json, (New-Object Text.UTF8Encoding($false)))

$env:SCD_CONFIG = $cfg
$env:SCD_COLOR  = 'always'
Remove-Item Env:\SCD_TARGET_FILE -ErrorAction SilentlyContinue
Remove-Item Env:\SCD_NO_HINT -ErrorAction SilentlyContinue
$inFile = Join-Path $work 'stdin.txt'

function Run([string[]]$arguments, [string]$stdin) {
    $argLine = ($arguments | ForEach-Object { '"' + $_ + '"' }) -join ' '
    if ($null -ne $stdin) {
        [IO.File]::WriteAllText($inFile, $stdin, (New-Object Text.UTF8Encoding($false)))
        $lines = cmd /c "$exe $argLine < `"$inFile`" 2>&1"
    } else {
        $lines = cmd /c "$exe $argLine 2>&1"
    }
    return (($lines | ForEach-Object { "$_" }) -join "`n")
}

$colors = @{
 '30'='#3b4045';'31'='#f14c4c';'32'='#4ec94e';'33'='#f5f543';'34'='#6cb6ff';'35'='#d670d6';'36'='#56b6c2';'37'='#d4d4d4'
 '90'='#7f8c96';'91'='#ff7b72';'92'='#7ee787';'93'='#ffe085';'94'='#79c0ff';'95'='#ff9bda';'96'='#56d4dd';'97'='#ffffff'
}
function To-Html([string]$text) {
    $sb = New-Object Text.StringBuilder
    foreach ($tok in [regex]::Split($text, "(\x1b\[[0-9;]*m)")) {
        if ($tok -match "^\x1b\[([0-9;]*)m$") {
            foreach ($code in ($matches[1] -split ';' | Where-Object { $_ -ne '' })) {
                switch ($code) {
                    '0' { $script:bold=$false;$script:dim=$false;$script:under=$false;$script:fg=$null }
                    '1' { $script:bold=$true }
                    '2' { $script:dim=$true }
                    '4' { $script:under=$true }
                    default { if ($colors.ContainsKey($code)) { $script:fg = $colors[$code] } }
                }
            }
            continue
        }
        if ($tok -eq '') { continue }
        $style = @()
        if ($script:fg) { $style += "color:$($script:fg)" }
        if ($script:bold) { $style += 'font-weight:700' }
        if ($script:dim) { $style += 'opacity:.62' }
        if ($script:under) { $style += 'text-decoration:underline' }
        $safe = [System.Net.WebUtility]::HtmlEncode($tok)
        if ($style.Count -eq 0) { [void]$sb.Append($safe) } else { [void]$sb.Append("<span style=`"$($style -join ';')`">$safe</span>") }
    }
    return $sb.ToString()
}

$sections = [ordered]@{
 'scd                      交互菜单（max_display=3，共 5 个位置）' = (Run @() "q`n")
 'scd 3                    按编号跳转（未装 shell 函数时的提示）'  = (Run @('3') $null)
 'scd list                 全部位置，含被隐藏的'                   = (Run @('list') $null)
 'scd  → a                 添加位置：先输错路径，再输对'            = (Run @() "a`n$work\no-such-dir`n$work\docs`nq`n")
 'scd  → c                 配置：先输非法值，再输 7'                = (Run @() "c`nabc`n7`nq`n")
 'scd nope                 解析失败：错误 + 提示'                   = (Run @('nope') $null)
 'scd --help               用户指南（前 44 行）'                    = (((Run @('--help') $null) -split "`n" | Select-Object -First 44) -join "`n")
}

$html = New-Object Text.StringBuilder
[void]$html.AppendLine('<!doctype html><html lang="zh"><meta charset="utf-8"><title>scd 0.2.0 配色预览</title>')
[void]$html.AppendLine('<style>body{background:#1e1e1e;color:#d4d4d4;font:13px/1.6 Consolas,"Cascadia Mono","Microsoft YaHei Mono",monospace;padding:24px}')
[void]$html.AppendLine('h1{font:600 18px Consolas;color:#4ec94e}h2{font:600 13px Consolas;color:#9cdcfe;margin:28px 0 6px;border-bottom:1px solid #333;padding-bottom:4px}')
[void]$html.AppendLine('pre{margin:0;white-space:pre-wrap}code{color:#6a9955}p{color:#808080}')
[void]$html.AppendLine('.light{background:#fff;color:#1f1f1f;padding:10px;border-radius:4px}</style>')
[void]$html.AppendLine('<h1>scd 0.2.0 — 配色预览</h1>')
[void]$html.AppendLine('<p>下面是真实二进制在 <code>SCD_COLOR=always</code> 下的输出（ANSI 已转成 CSS）。终端里实际观感还取决于你的配色方案；重定向/管道时会自动关闭颜色。</p>')
foreach ($k in $sections.Keys) {
    [void]$html.AppendLine('<h2>' + [System.Net.WebUtility]::HtmlEncode($k) + '</h2>')
    [void]$html.AppendLine('<pre>' + (To-Html $sections[$k]) + '</pre>')
}
[void]$html.AppendLine('<h2>同一份输出放在浅色背景下（标题用粗体+下划线，不依赖亮色）</h2>')
[void]$html.AppendLine('<div class="light"><pre>' + (To-Html (Run @('list') $null)) + '</pre></div>')
[IO.File]::WriteAllText("$repo\target\preview.html", $html.ToString(), (New-Object Text.UTF8Encoding($false)))
Write-Output "preview.html = $((Get-Item "$repo\target\preview.html").Length) bytes"
Write-Output "--- 原始转义抽样（菜单前两行）---"
((Run @() "q`n") -split "`n" | Select-Object -First 8) | ForEach-Object { $_ -replace "`e", '<ESC>' }