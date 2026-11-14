$ErrorActionPreference = "Stop"
$src = [System.IO.File]::ReadAllText("$PSScriptRoot/../src-tauri/src/services/models/catalog.rs")
$entryRegex = [regex]'entry\(\s*"([^"]*)",\s*"([^"]*)",\s*"([^"]*)",\s*"([^"]*)",\s*"([^"]*)",\s*([\d_]+),\s*([\d.]+),\s*"([^"]*)",\s*"([0-9a-f]{64})",\s*"([^"]*)",\s*"([^"]*)",\s*(vec!\[[\s\S]*?\]|vec!\[\])\s*,\s*\)\s*(?:\.with_context\(([\d_]+)\)\s*)?(?:\.with_dest\("([^"]*)"\)\s*)?'
$m = $entryRegex.Matches($src) | Where-Object { $_.Groups[1].Value -eq "qwen3.5-9b-q4" } | Select-Object -First 1
Write-Output ("g13 success: " + $m.Groups[13].Success + " value: [" + $m.Groups[13].Value + "]")
# Show the raw tail: what follows the vec group close
$idx = $m.Groups[12].Index + $m.Groups[12].Length
$tail = $src.Substring($idx, 120).Replace("`r", "").Replace("`n", "\n")
Write-Output ("tail after vec: " + $tail)
