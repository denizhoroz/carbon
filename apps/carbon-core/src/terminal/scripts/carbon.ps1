# Carbon shell integration for PowerShell (7 and Windows PowerShell 5.1).
# Marks prompts and commands with OSC 633 so Carbon knows where each command
# starts and ends, its exit code and the working directory.
if ($Global:__CarbonShellIntegration) { return }
$Global:__CarbonShellIntegration = $true

$Global:__CarbonEsc = [char]0x1b
$Global:__CarbonBel = [char]0x07
$Global:__CarbonLastHistoryId = -1
$Global:__CarbonOriginalPrompt = $function:Prompt

function Global:__CarbonEscape([string]$value) {
    $out = New-Object System.Text.StringBuilder
    foreach ($ch in $value.ToCharArray()) {
        if ($ch -eq '\') { [void]$out.Append('\\') }
        elseif ([int]$ch -lt 0x20 -or $ch -eq ';') { [void]$out.Append(('\x{0:x2}' -f [int]$ch)) }
        else { [void]$out.Append($ch) }
    }
    $out.ToString()
}

function Global:__CarbonOsc([string]$body) {
    "$Global:__CarbonEsc]633;$body$Global:__CarbonBel"
}

function Global:Prompt {
    # Read the last command's result before anything else changes it.
    $succeeded = $global:?
    $nativeCode = $global:LASTEXITCODE
    $out = ''
    $last = Get-History -Count 1
    if ($Global:__CarbonLastHistoryId -ne -1) {
        if ($last -and $last.Id -ne $Global:__CarbonLastHistoryId) {
            $code = if ($succeeded) { 0 } elseif ($nativeCode) { $nativeCode } else { 1 }
            $out += __CarbonOsc "D;$code"
        } else {
            $out += __CarbonOsc 'D'
        }
    }
    $Global:__CarbonLastHistoryId = if ($last) { $last.Id } else { 0 }
    $out += __CarbonOsc 'A'
    $out += __CarbonOsc "P;Cwd=$(__CarbonEscape $executionContext.SessionState.Path.CurrentLocation.ProviderPath)"
    $out += & $Global:__CarbonOriginalPrompt
    $out += __CarbonOsc 'B'
    $out
}

# The command line and the moment it runs come from the Enter key.
if (Get-Module -Name PSReadLine) {
    Set-PSReadLineKeyHandler -Chord Enter -ScriptBlock {
        $line = $null
        $cursor = $null
        [Microsoft.PowerShell.PSConsoleReadLine]::GetBufferState([ref]$line, [ref]$cursor)
        [Console]::Write((__CarbonOsc "E;$(__CarbonEscape $line)") + (__CarbonOsc 'C'))
        [Microsoft.PowerShell.PSConsoleReadLine]::AcceptLine()
    }
}
