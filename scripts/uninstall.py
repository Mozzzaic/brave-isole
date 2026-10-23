#!/usr/bin/env python3
"""Remove owned integration entries; keep unrelated edits and Brave data."""
import argparse,json,socket,shutil
from pathlib import Path
from configure_clients import ROOT,digest,restore_owned

def main():
 parser=argparse.ArgumentParser();mode=parser.add_mutually_exclusive_group(required=True);mode.add_argument('--dry-run',action='store_true');mode.add_argument('--apply',action='store_true');args=parser.parse_args()
 path=ROOT/'work/install-receipt.json'
 if not path.exists():print(json.dumps({'status':'nothing-installed-by-this-installer'}));return
 receipt=json.loads(path.read_text())
 if args.dry_run:print(json.dumps({'status':'dry-run','paths':[item['path'] for item in receipt],'brave_profile_modified':False,'playwriter_reactivated':False},indent=2));return
 endpoint=Path.home()/'Library/Containers/dev.david.brave-isole/Data/b/s'
 if endpoint.exists():
  with socket.socket(socket.AF_UNIX) as check:
   try:check.connect(str(endpoint))
   except ConnectionRefusedError:pass
   else:raise RuntimeError('stop the extension before uninstalling')
 verification=endpoint.parent/'verification.json'
 verification.unlink(missing_ok=True)
 for item in receipt:
  target=Path(item['path'])
  if not target.exists():continue
  if item['kind']=='native':
   if digest(target.read_bytes())!=item['after_hash']:raise RuntimeError('native manifest modified; left intact')
   # Original bytes remain in the private installation backup.
   target.unlink()
  else:restore_owned(target,item['entry'],item['kind'])
 release=json.loads((ROOT/'work/release.json').read_text());app=Path(release.get('installation_target',release['executable'])).parents[2]
 if app.name=='BraveIsole.app' and app.is_dir() and app.parent==Path.home()/'Library/Application Support/BraveIsole':
  executable=app/'Contents/MacOS/brave-isole'
  if digest(executable.read_bytes())!=release['sha256']:raise RuntimeError('installed app modified; left intact')
  destination=ROOT/'work/private-backups/uninstalled-BraveIsole.app'
  if destination.exists():raise RuntimeError('previous uninstall backup exists; left intact')
  destination.parent.mkdir(mode=0o700,parents=True,exist_ok=True);shutil.move(str(app),str(destination))
 print(json.dumps({'status':'integration-disabled','brave_profile_modified':False,'extension_removal':'Remove only Brave isole in Brave extensions if desired.'}))
if __name__=='__main__':
 try:main()
 except (OSError,ValueError,RuntimeError):raise SystemExit('Uninstall needs review; no unrelated browser data was targeted.')
