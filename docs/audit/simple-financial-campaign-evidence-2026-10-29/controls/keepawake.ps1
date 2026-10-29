param()
$ErrorActionPreference = 'Stop'
$taskRunFolder = $PSScriptRoot
$taskLinuxRoot = '\\wsl.localhost\ubuntu-24.04\root\fuzz\pooled-110-financial-20261029T080421Z-8c50fa75'
$taskStatePath = Join-Path $taskRunFolder 'keepawake-state.json'
$taskLaunchPath = Join-Path $taskRunFolder 'wsl-launch.json'
Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
public static class ShortwinAwake {
 [DllImport("kernel32.dll", SetLastError=true)] public static extern uint SetThreadExecutionState(uint flags);
 [StructLayout(LayoutKind.Sequential)] public struct PowerStatus { public byte ACLineStatus; public byte BatteryFlag; public byte BatteryLifePercent; public byte SystemStatusFlag; public uint BatteryLifeTime; public uint BatteryFullLifeTime; }
 [DllImport("kernel32.dll")] public static extern bool GetSystemPowerStatus(out PowerStatus status);
}
'@
$taskStarted = [DateTimeOffset]::UtcNow
$taskPrevious = [ShortwinAwake]::SetThreadExecutionState([uint32]2147483649)
if ($taskPrevious -eq 0) { throw 'Windows system-awake request failed' }
$taskState = @{ schema=1; process_id=$PID; started_at_utc=$taskStarted.ToString('o'); initial_api_return=$taskPrevious; system_required=$true; reason='holding' }
function Write-TaskState {
 $taskPower = New-Object ShortwinAwake+PowerStatus
 $taskPowerOk = [ShortwinAwake]::GetSystemPowerStatus([ref]$taskPower)
 $taskState.at_utc = [DateTimeOffset]::UtcNow.ToString('o')
 $taskState.ac_line_status = if ($taskPowerOk) { [int]$taskPower.ACLineStatus } else { $null }
 $taskState.battery_percent = if ($taskPowerOk) { [int]$taskPower.BatteryLifePercent } else { $null }
 $taskTemporary = $taskStatePath + '.tmp'
 [IO.File]::WriteAllText($taskTemporary,($taskState | ConvertTo-Json -Depth 6),[Text.UTF8Encoding]::new($false))
 Move-Item -LiteralPath $taskTemporary -Destination $taskStatePath -Force
}
try {
 while ($true) {
  Write-TaskState
  if ((Test-Path -LiteralPath (Join-Path $taskLinuxRoot 'outer-completion.json')) -or (Test-Path -LiteralPath (Join-Path $taskLinuxRoot 'supervisor-error.json'))) { $taskState.reason='supervisor_finished'; break }
  if (Test-Path -LiteralPath $taskLaunchPath) {
   $taskLaunch = Get-Content -LiteralPath $taskLaunchPath -Raw | ConvertFrom-Json
   $taskProcess = Get-Process -Id $taskLaunch.process_id -ErrorAction SilentlyContinue
   if (($null -eq $taskProcess) -or ($taskProcess.StartTime.ToUniversalTime().Ticks.ToString() -ne $taskLaunch.start_time_ticks)) { $taskState.reason='launcher_exited_without_completion'; break }
  } elseif (([DateTimeOffset]::UtcNow - $taskStarted).TotalMinutes -gt 10) { $taskState.reason='launch_not_observed'; break }
  [Threading.Thread]::Sleep(15000)
 }
} catch { $taskState.reason='helper_error'; $taskState.error=$_.Exception.Message; throw }
finally {
 [void][ShortwinAwake]::SetThreadExecutionState([uint32]2147483648)
 $taskState.system_required=$false
 Write-TaskState
}
