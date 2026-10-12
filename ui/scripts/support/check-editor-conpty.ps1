param([Parameter(Mandatory=$true)][string]$ProjectRoot, [ValidateSet('success','failure','interrupt','backend-death')][string]$Case='success')
$ErrorActionPreference='Stop'
$ProjectRoot=[IO.Path]::GetFullPath($ProjectRoot)
$Backend=[IO.Path]::GetFullPath((Join-Path $ProjectRoot 'src-tauri/target/debug/norn-backend.exe'))
$Bun=(Get-Command bun).Source; $Git=(Get-Command git).Source; $Rustc=(Get-Command rustc).Source
if (!(Test-Path $Backend)) { throw 'Required native backend is missing' }
Add-Type -Path (Join-Path $ProjectRoot 'ui/scripts/support/owned-conpty.cs')
$Root=Join-Path ([IO.Path]::GetTempPath()) ('norn-owned-editor-conpty-'+[Guid]::NewGuid().ToString('N'))
$Repo=Join-Path $Root 'checkout with spaces';$Config=Join-Path $Root 'config';$Editors=Join-Path $Root 'editors'
$null=New-Item -ItemType Directory -Path $Repo,$Config,$Editors
$Session=$null
try {
  $Editor=Join-Path $Editors 'nvim.exe';$Witness=Join-Path $Editors 'console-witness.exe'
  & $Rustc --edition=2021 (Join-Path $ProjectRoot 'ui/scripts/support/owned-editor.rs') -o $Editor
  if ($LASTEXITCODE -ne 0) { throw 'Owned editor compilation failed' }
  & $Rustc --edition=2021 (Join-Path $ProjectRoot 'ui/scripts/support/owned-console-witness.rs') -o $Witness
  if ($LASTEXITCODE -ne 0) { throw 'Owned console witness compilation failed' }
  $env:GIT_CONFIG_GLOBAL='NUL';$env:GIT_CONFIG_NOSYSTEM='1'
  function OwnedGit {
    param([Parameter(Mandatory=$true)][string[]]$GitArgs)
    & $Git -C $Repo @GitArgs
    if ($LASTEXITCODE -ne 0) {throw 'Owned Git setup failed'}
  }
  # Pass literal argv through a named array. A bare `--` at a PowerShell
  # function call is syntax and disappears before it reaches native Git.
  OwnedGit -GitArgs @('init','-q')
  OwnedGit -GitArgs @('config','user.name','Owned Editor')
  OwnedGit -GitArgs @('config','user.email','owned@example.invalid')
  OwnedGit -GitArgs @('remote','add','origin','https://github.com/owned/editor.git')
  $Source=Join-Path $Repo '--owned ü.txt'
  [IO.File]::WriteAllText($Source,"owned line one`nsecond line`n",[Text.UTF8Encoding]::new($false))
  OwnedGit -GitArgs @('add','--','--owned ü.txt')
  OwnedGit -GitArgs @('-c','commit.gpgsign=false','commit','-qm','owned base')
  $Settings=@{aiProvider='claude';repos=@(@{provider='github';workspace='owned';repo='editor';localPath=$Repo});defaultDiffView='unified';theme='dark'}|ConvertTo-Json -Depth 5
  [IO.File]::WriteAllText((Join-Path $Config 'settings.json'),$Settings,[Text.UTF8Encoding]::new($false))
  # Deliberately bounded environment: no personal config/profile or credentials.
  $Environment=@{PATH=($Editors+';'+[IO.Path]::GetDirectoryName($Git)+';'+$env:SystemRoot+'\System32;'+$env:SystemRoot);SystemRoot=$env:SystemRoot;WINDIR=$env:WINDIR;TEMP=$Root;TMP=$Root;TERM='xterm-256color';NORN_BACKEND_BIN=$Backend;NORN_BACKEND_REPO_PATH=$Repo;NORN_CONFIG_DIR=$Config;NORN_DATA_DIR=(Join-Path $Root 'workspace');NORN_REVIEW_DATA_DIR=(Join-Path $Root 'reviews');NORN_OWNED_EDITOR_DIR=$Editors;VISUAL=$Editor;GIT_CONFIG_NOSYSTEM='1';GIT_CONFIG_GLOBAL='NUL';NORN_BROWSER_OPEN='0'}
  $Block=($Environment.Keys|Sort-Object|ForEach-Object {$_+'='+$Environment[$_]}) -join "`0"
  $Session=[OwnedConPty]::new($Witness,('"'+$Bun+'" run src/main.ts'),(Join-Path $ProjectRoot 'ui'),($Block+"`0`0"))
  function AwaitOwned([scriptblock]$Condition,[string]$Label,[bool]$AllowExited=$false) {
    $Deadline=[DateTime]::UtcNow.AddSeconds(25)
    while (!( & $Condition )) {
      if([DateTime]::UtcNow -gt $Deadline -or (!$AllowExited -and $Session.ExitCode -ne -1)) {
        $Facts=@{witnessExit=$Session.ExitCode}
        $ModesFile=Join-Path $Editors 'changed-modes.txt'
        if(Test-Path $ModesFile){$Facts.editorModes=[IO.File]::ReadAllText($ModesFile)}
        $ConsoleModesFile=Join-Path $Editors 'console-modes.txt'
        if(Test-Path $ConsoleModesFile){$Facts.consoleModes=[IO.File]::ReadAllText($ConsoleModesFile)}
        $PidFile=Join-Path $Editors 'editor.pid'
        if(Test-Path $PidFile){$Facts.editorAlive=[bool](Get-Process -Id ([uint32][IO.File]::ReadAllText($PidFile)) -ErrorAction SilentlyContinue)}
        $InputFile=Join-Path $Editors 'input.txt'
        if(Test-Path $InputFile){$Facts.editorInputHex=[Convert]::ToHexString([IO.File]::ReadAllBytes($InputFile))}
        throw ($Label+"`n"+($Facts|ConvertTo-Json -Compress)+"`n"+$Session.Output)
      }
      Start-Sleep -Milliseconds 25
    }
  }
  AwaitOwned { $Session.Output.Contains('F8') } 'Production workspace did not render'
  $Session.Send("`e[19~")
  AwaitOwned { $Session.Output.Contains('--owned ü.txt') } 'F8 inventory did not render'
  $Session.Send("`r")
  AwaitOwned { $Session.Output.Contains('owned line one') } 'Selected source did not render'
  AwaitOwned { $Session.Output.Contains('o open') } 'Workspace did not finish initial loading'
  $Session.Send('O')
  AwaitOwned { $Session.Output.Contains('OWNED_EDITOR_READY') } 'Editor did not acquire real console handles'
  if (!(Test-Path (Join-Path $Editors 'changed-modes.txt'))) {throw 'Editor did not change native console modes'}
  $Actual=[IO.File]::ReadAllText((Join-Path $Editors 'argv.txt')).Replace('\\?\','')
  if ($Actual -ne ('+1'+"`n"+$Source)) {throw ('Editor argv changed: '+$Actual)}
  $EditorPid=[uint32][IO.File]::ReadAllText((Join-Path $Editors 'editor.pid'))
  $UiPid=[uint32][IO.File]::ReadAllText((Join-Path $Editors 'ui.pid'))
  $Expected=0
  if ($Case -eq 'backend-death') {
    $WrapperPid=[OwnedConPty]::Parent($EditorPid);$BackendPid=[OwnedConPty]::Parent($WrapperPid)
    $Process=Get-Process -Id $BackendPid
    if ([OwnedConPty]::Parent($BackendPid) -ne $UiPid -or $Process.Path -ne $Backend) {throw 'Death test does not own this backend'}
    $Process.Kill()
    if(!$Process.WaitForExit(5000)){throw 'Owned backend did not terminate'}
    $BackendExit=$Process.ExitCode
    # Bun's process.exit preserves the low exit-status byte. Derive it from
    # this actual owned native process, rather than guessing Kill's status.
    $Expected=$BackendExit -band 255
  } else {
    if ($Case -eq 'interrupt') {$Session.Send([string][char]3)}
    elseif ($Case -eq 'failure') {$Session.Send("fail`r`n")}
    else {$Session.Send("finish`r`n")}
    if ($Case -ne 'interrupt') {AwaitOwned { Test-Path (Join-Path $Editors 'input.txt') } 'Editor did not read console input'}
    AwaitOwned { $Session.Output.Contains('nvim · exited') -or $Session.Output.Contains('nvim · failed') -or $Session.Output.Contains('interrupted') -or $Session.Output.Contains('exit code') -or $Session.Output.Contains('exited unsuccessfully') } 'UI did not resume after editor exit'
    $Session.Send('q')
  }
  AwaitOwned { $Session.ExitCode -ne -1 } 'Production UI did not quit'
  if ($Session.ExitCode -ne $Expected) {throw ('Unexpected UI exit '+$Session.ExitCode)}
  if($Case -eq 'backend-death' -and !$Session.Output.Contains('norn-backend exited with code '+$BackendExit+'.')){throw 'UI did not report the actual backend exit cause'}
  AwaitOwned { !(Get-Process -Id $EditorPid -ErrorAction SilentlyContinue) } 'Owned editor remained orphaned' $true
  $Modes=[IO.File]::ReadAllLines((Join-Path $Editors 'console-modes.txt'))
  if($Modes.Count -ne 2 -or $Modes[0] -ne $Modes[1]){throw 'Native console modes were not restored'}
  Write-Output ('PASS '+$Case+': production OpenTUI, literal argv, real ConPTY input, editor cleanup and native mode restoration')
} finally {
  if ($Session) {
    # This is the witness process created above, including its own UI descendants.
    $Owned=Get-Process -Id $Session.Pid -ErrorAction SilentlyContinue
    if($Owned -and $Owned.Path -eq $Witness){$Owned.Kill($true);$Owned.WaitForExit(5000)|Out-Null}
    $Session.Dispose()
  }
  for($Attempt=0;$Attempt -lt 20;$Attempt++) {
    try {Remove-Item -LiteralPath $Root -Recurse -Force;break}
    catch {if($Attempt -eq 19){throw};Start-Sleep -Milliseconds 100}
  }
}
