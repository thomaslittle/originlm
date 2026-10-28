"""Uninstall only a throwaway prefix; never attach to the user's service manager."""
import os
import pathlib
import subprocess
import tempfile
import unittest

ROOT = pathlib.Path(__file__).resolve().parents[2]

@unittest.skipIf(os.name == 'nt', 'Unix shell installer')
class UnixUninstall(unittest.TestCase):
    def run_uninstall(self, platform='Linux', purge=False):
        temporary = tempfile.TemporaryDirectory(prefix='originlm-uninstall-')
        self.addCleanup(temporary.cleanup)
        base = pathlib.Path(temporary.name)
        home, shims, state, data, config = [base/name for name in ('home','shims','service-state','data','config')]
        for path in (home, shims, state, data, config): path.mkdir()
        for name, text in [('uname', f'#!/bin/sh\nprintf "{platform}\\n"\n'), ('systemctl', '#!/bin/sh\nexit 0\n'), ('launchctl', '#!/bin/sh\nexit 0\n')]:
            path=shims/name;path.write_text(text);path.chmod(0o755)
        runtime_data = home/'Library/Application Support/originlm' if platform=='Darwin' else data/'originlm'
        runtime_data.mkdir(parents=True)
        (runtime_data/'observations.db').write_text('fixture')
        (state/'keep.txt').write_text('fixture')
        manifest_root = home/'Library/Application Support/Google/Chrome/NativeMessagingHosts' if platform=='Darwin' else config/'google-chrome/NativeMessagingHosts'
        manifest_root.mkdir(parents=True)
        actual = manifest_root/'com.originlm.host.json';actual.write_text('{}')
        unrelated = manifest_root/'another-app.json';unrelated.write_text('{}')
        env={**os.environ,'HOME':str(home),'PATH':str(shims)+os.pathsep+os.environ['PATH'],'LAI_STATE_DIR':str(state),'LAI_BIN_DIR':str(base/'bin'),'XDG_CONFIG_HOME':str(config),'XDG_STATE_HOME':str(data)}
        env.pop('LAI_DATA_DIR',None);env.pop('LOCAL_AI_KIT_STATE_DIR',None)
        result=subprocess.run(['sh',str(ROOT/'scripts/service/uninstall.sh'),*(['--purge'] if purge else [])],env=env,capture_output=True,text=True)
        self.assertEqual(result.returncode,0,result.stderr)
        self.assertFalse(actual.exists(),'the actual registered host must be removed')
        self.assertTrue(unrelated.exists(),'unrelated registrations must remain')
        self.assertEqual(runtime_data.exists(),not purge)
        self.assertEqual((state/'keep.txt').exists(),not purge)

    def test_normal_uninstall_removes_registration_and_retains_data(self):
        self.run_uninstall()
    def test_purge_respects_xdg_state_and_config(self):
        self.run_uninstall(purge=True)
    def test_macos_purge_matches_the_runtime_support_directory(self):
        self.run_uninstall(platform='Darwin',purge=True)

    def test_purge_rejects_a_home_alias_before_removing_data(self):
        with tempfile.TemporaryDirectory(prefix='originlm-uninstall-') as directory:
            home=pathlib.Path(directory);sentinel=home/'keep.txt';sentinel.write_text('fixture')
            env={**os.environ,'HOME':directory,'LAI_STATE_DIR':directory+'/.','LAI_DATA_DIR':str(home/'data'),'LAI_BIN_DIR':str(home/'bin'),'XDG_CONFIG_HOME':str(home/'config')}
            result=subprocess.run(['sh',str(ROOT/'scripts/service/uninstall.sh'),'--purge'],env=env,capture_output=True,text=True)
            self.assertEqual(result.returncode,2)
            self.assertTrue(sentinel.exists())

@unittest.skipUnless(os.name == 'nt', 'Windows PowerShell fixture')
class WindowsUninstall(unittest.TestCase):
    def test_real_filesystem_removal_with_stubbed_registry_and_task_operations(self):
        import shutil
        with tempfile.TemporaryDirectory(prefix='originlm-uninstall-') as directory:
            base=pathlib.Path(directory);script=base/'fixture.ps1'
            script.write_text(r'''$ErrorActionPreference = 'Stop'
$fixtureRoot = $env:ORIGINLM_FIXTURE_ROOT
$install = Join-Path $fixtureRoot 'bin'
$state = Join-Path $fixtureRoot 'state'
$data = Join-Path $fixtureRoot 'runtime-data'
$log = Join-Path $fixtureRoot 'external-operations.txt'
New-Item -ItemType Directory -Force $install,$state,$data | Out-Null
Set-Content (Join-Path $install 'originlm.exe') 'fixture'
Set-Content (Join-Path $state 'keep.txt') 'fixture'
Set-Content (Join-Path $data 'observations.db') 'fixture'
function Stop-ScheduledTask { [CmdletBinding()] param([string]$TaskName) }
function Unregister-ScheduledTask { [CmdletBinding(SupportsShouldProcess)] param([string]$TaskName) }
function Remove-Item {
  [CmdletBinding()] param([Parameter(Position=0)][string[]]$Path,[switch]$Force,[switch]$Recurse)
  foreach ($item in $Path) {
    if ($item.StartsWith($fixtureRoot, [StringComparison]::OrdinalIgnoreCase)) {
      Microsoft.PowerShell.Management\Remove-Item -LiteralPath $item -Force:$Force -Recurse:$Recurse -ErrorAction SilentlyContinue
    } else { Add-Content -LiteralPath $log -Value $item }
  }
}
& $env:ORIGINLM_UNINSTALL -InstallDir $install -StateDir $state -DataDir $data
if ((Test-Path (Join-Path $install 'originlm.exe')) -or -not (Test-Path (Join-Path $state 'keep.txt')) -or -not (Test-Path (Join-Path $data 'observations.db'))) { throw 'Ordinary removal did not retain fixture data' }
& $env:ORIGINLM_UNINSTALL -InstallDir $install -StateDir $state -DataDir $data -Purge
if ((Test-Path $state) -or (Test-Path $data)) { throw 'Purge did not remove fixture state/data' }
$operations=Get-Content $log
foreach ($browser in @('Google\Chrome','Chromium','Microsoft\Edge','Mozilla')) {
 if ($operations -notcontains "HKCU:\Software\$browser\NativeMessagingHosts\com.originlm.host") { throw 'Canonical native-host registry removal was missing' }
}
''',encoding='utf-8')
            executable=shutil.which('powershell') or shutil.which('pwsh')
            self.assertIsNotNone(executable)
            result=subprocess.run([executable,'-NoProfile','-NonInteractive','-ExecutionPolicy','Bypass','-File',str(script)],env={**os.environ,'ORIGINLM_FIXTURE_ROOT':directory,'ORIGINLM_UNINSTALL':str(ROOT/'scripts/service/uninstall.ps1')},capture_output=True,text=True)
            self.assertEqual(result.returncode,0,result.stdout+result.stderr)
