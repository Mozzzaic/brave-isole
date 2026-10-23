#!/usr/bin/env python3
"""Build and sign the local app. Does not install or enable browser permissions."""
import base64,hashlib,json,os,plistlib,shutil,subprocess
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]
def run(*args,**kwargs):return subprocess.run(args,check=True,cwd=ROOT,**kwargs)
def main():
 public=ROOT/'packaging/extension-public-key.der'
 if not public.exists():
  private=run('/usr/bin/openssl','genpkey','-algorithm','RSA','-pkeyopt','rsa_keygen_bits:2048',stdout=subprocess.PIPE,stderr=subprocess.PIPE).stdout
  public.write_bytes(run('/usr/bin/openssl','pkey','-pubout','-outform','DER',input=private,stdout=subprocess.PIPE,stderr=subprocess.PIPE).stdout)
 key=public.read_bytes();extension_id=''.join(chr(ord('a')+int(char,16)) for char in hashlib.sha256(key).hexdigest()[:32])
 manifest=json.loads((ROOT/'extension/manifest.json').read_text());manifest['key']=base64.b64encode(key).decode();(ROOT/'extension/manifest.json').write_text(json.dumps(manifest,ensure_ascii=False,indent=2)+'\n')
 run(str(Path.home()/'.cargo/bin/cargo'),'build','--release','--locked','--bin','brave-isole')
 app=ROOT/'work/BraveIsole.app';macos=app/'Contents/MacOS';macos.mkdir(parents=True,exist_ok=True)
 info={'CFBundleIdentifier':'dev.david.brave-isole','CFBundleName':'Brave isole','CFBundleExecutable':'brave-isole','CFBundlePackageType':'APPL','CFBundleShortVersionString':'0.1.0','CFBundleVersion':'1','LSUIElement':True,'LSMinimumSystemVersion':'13.0'}
 (app/'Contents/Info.plist').write_bytes(plistlib.dumps(info));exe=macos/'brave-isole';shutil.copy2(ROOT/'target/release/brave-isole',exe)
 run('/usr/bin/codesign','--force','--sign','-','--identifier','dev.david.brave-isole','--options','runtime','--entitlements',str(ROOT/'packaging/app.entitlements'),str(app))
 run('/usr/bin/codesign','--verify','--strict','--verbose=2',str(app))
 run(str(exe),'--initialize',extension_id)
 metadata={'executable':str(exe),'sha256':hashlib.sha256(exe.read_bytes()).hexdigest(),'extension_id':extension_id,'bundle_id':'dev.david.brave-isole','installed':False,'installation_target':str(Path.home()/'Library/Application Support/BraveIsole/BraveIsole.app/Contents/MacOS/brave-isole')}
 p=ROOT/'work/release.json';p.write_text(json.dumps(metadata,indent=2)+'\n');p.chmod(0o600)
 print(json.dumps(metadata,indent=2))
if __name__=='__main__':main()
