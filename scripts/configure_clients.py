#!/usr/bin/env python3
"""Reversible targeted MCP configuration. Requires Python >= 3.11."""
import argparse,hashlib,json,os,re,tempfile,tomllib,shutil,subprocess,secrets,time,socket,sys
from datetime import datetime,timezone
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]
KEY='brave_isole'
START='# brave-isole managed start'
END='# brave-isole managed end'
def digest(value):return hashlib.sha256(value).hexdigest()
def encode(value):return (json.dumps(value,ensure_ascii=False,indent=2)+'\n').encode()
def change(path,before,after,kind,entry):return {'path':Path(path),'before':before,'after':after,'hash':digest(before),'kind':kind,'entry':entry}
def plan_json(path,entry,owned_entry=None):
 path=Path(path)
 if not path.exists():return None
 before=path.read_bytes();value=json.loads(before)
 if not isinstance(value,dict) or not isinstance(value.get('mcpServers',{}),dict):raise ValueError('invalid MCP configuration')
 servers=value.setdefault('mcpServers',{})
 if KEY in servers:
  if servers[KEY]==entry:return None
  if servers[KEY]!=owned_entry:raise RuntimeError('existing brave_isole entry differs')
 servers[KEY]=entry
 return change(path,before,encode(value),'json',entry)
def toml_block(entry):
 return '\n'+START+'\n[mcp_servers.brave_isole]\n'+''.join(k+' = '+(str(v).lower() if isinstance(v,bool) else json.dumps(v,ensure_ascii=False))+'\n' for k,v in entry.items())+END+'\n'
def plan_toml(path,entry,owned_entry=None):
 path=Path(path)
 if not path.exists():return None
 before=path.read_bytes();value=tomllib.loads(before.decode())
 old=value.get('mcp_servers',{}).get(KEY)
 if old is not None:
  if old==entry:return None
  if old!=owned_entry:raise RuntimeError('existing brave_isole entry differs')
  prior=toml_block(old)
  if before.decode().count(prior)!=1:raise RuntimeError('managed TOML block changed')
  after=before.decode().replace(prior,toml_block(entry),1)
 else:after=before.decode().rstrip()+'\n'+toml_block(entry)
 updated=tomllib.loads(after)
 expected={**value,'mcp_servers':{**value.get('mcp_servers',{}),KEY:entry}}
 if updated!=expected:raise RuntimeError('unexpected TOML change')
 return change(path,before,after.encode(),'toml',entry)
def atomic_write(path,data,expected):
 path=Path(path)
 current=path.read_bytes() if path.exists() else b''
 if digest(current)!=expected:raise RuntimeError('configuration changed; rerun dry-run')
 fd,name=tempfile.mkstemp(prefix='.brave-isole-',dir=path.parent)
 try:
  os.fchmod(fd,0o600)
  with os.fdopen(fd,'wb') as stream:stream.write(data);stream.flush();os.fsync(stream.fileno())
  current=path.read_bytes() if path.exists() else b''
  if digest(current)!=expected:raise RuntimeError('configuration changed during preparation')
  os.replace(name,path)
 finally:
  if os.path.exists(name):os.unlink(name)
def apply_changes(changes,backup):
 changes=[item for item in changes if item is not None];backup=Path(backup)
 if not changes:return []
 for item in changes:
  current=item['path'].read_bytes() if item['path'].exists() else b''
  if digest(current)!=item['hash']:raise RuntimeError('configuration changed; aborted')
 backup.mkdir(parents=True,mode=0o700,exist_ok=True);backup.chmod(0o700)
 receipt=[]
 for index,item in enumerate(changes):
  saved=backup/f'{index}.before';saved.write_bytes(item['before']);saved.chmod(0o600)
  receipt.append({'path':str(item['path']),'kind':item['kind'],'entry':item['entry'],'before_hash':item['hash'],'after_hash':digest(item['after']),'backup':str(saved)})
 record=backup/'receipt.json';record.write_bytes(encode(receipt));record.chmod(0o600)
 written=[]
 try:
  for item in changes:atomic_write(item['path'],item['after'],item['hash']);written.append(item)
 except Exception:
  for item in reversed(written):
   if digest(item['path'].read_bytes())==digest(item['after']):atomic_write(item['path'],item['before'],digest(item['after']))
  raise
 return receipt
def merge_receipts(previous,incoming):
 merged={item['path']:dict(item) for item in previous}
 for item in incoming:
  old=merged.get(item['path'])
  merged[item['path']]={**item,**({'before_hash':old['before_hash'],'backup':old['backup']} if old else {})}
 return list(merged.values())
def restore_owned(path,entry,kind):
 path=Path(path);before=path.read_bytes()
 if kind=='json':
  value=json.loads(before);servers=value.get('mcpServers',{})
  if KEY not in servers:return
  if servers[KEY]!=entry:raise RuntimeError('user modified entry; left intact')
  del servers[KEY];after=encode(value)
 elif kind=='toml':
  value=tomllib.loads(before.decode());current=value.get('mcp_servers',{}).get(KEY)
  if current is None:return
  if current!=entry:raise RuntimeError('user modified entry; left intact')
  text=before.decode();pattern=r'\n?'+re.escape(START)+r'\n[\s\S]*?'+re.escape(END)+r'\n?'
  after=re.sub(pattern,'\n',text,count=1).encode()
  expected={**value,'mcp_servers':{k:v for k,v in value.get('mcp_servers',{}).items() if k!=KEY}}
  if tomllib.loads(after.decode())!=expected:raise RuntimeError('unrecognized managed TOML block')
 else:raise ValueError('unknown configuration type')
 atomic_write(path,after,digest(before))
def host_manifest(executable,extension_id):
 if not re.fullmatch('[a-p]{32}',extension_id):raise ValueError('invalid extension ID')
 return {'name':'dev.david.brave_isole','description':'Brave isole local sandboxed bridge','path':str(executable),'type':'stdio','allowed_origins':[f'chrome-extension://{extension_id}/']}
def metadata():
 value=json.loads((ROOT/'work/release.json').read_text());executable=Path(value['executable'])
 if not executable.is_file() or digest(executable.read_bytes())!=value['sha256']:raise RuntimeError('release executable changed')
 return value

REQUIRED_CHECKS={'read_denied','write_denied','tcp4_denied','tcp6_denied','udp4_denied','udp6_denied','container','private_ipc'}
def valid_report(role,report,request,release):
 if report.get('passed') is not True or report.get('role')!=role:return False
 if report.get('nonce')!=request.get('nonce') or report.get('binary_sha256')!=release['sha256'] or report.get('extension_id')!=release['extension_id']:return False
 checks=report.get('checks',{})
 return set(checks)==REQUIRED_CHECKS and all(value is True for value in checks.values())
def validate_reports(reports,request,release):
 return all(valid_report(role,reports.get(role,{}),request,release) for role in ['native','codex','claude'])

def parent_chain_matches(pid,prefix):
 for _ in range(10):
  if not isinstance(pid,int) or pid<=1:return False
  result=subprocess.run(['/bin/ps','-ww','-p',str(pid),'-o','ppid=,comm='],capture_output=True,text=True)
  parts=result.stdout.strip().split(None,1)
  if len(parts)!=2:return False
  if parts[1].startswith(str(prefix)+'/'):return True
  try:pid=int(parts[0])
  except ValueError:return False
 return False

def controls_alive(request):
 try:
  for key in ['tcp4','tcp6']:
   host,port=request[key].rsplit(':',1)
   with socket.create_connection((host.strip('[]'),int(port)),timeout=1):pass
  return True
 except (OSError,ValueError,KeyError):return False

def select_real_proofs(base,request,release):
 reports={}
 prefixes={'native':(Path('/Applications/Brave Browser.app'),),'codex':(Path('/Applications/Codex.app'),Path('/Applications/ChatGPT.app')),'claude':(Path('/Applications/Claude.app'),)}
 for role,allowed in prefixes.items():
  for path in sorted(base.glob(f'proof-{role}-*.json'),key=lambda path:path.stat().st_mtime,reverse=True)[:128]:
   report=json.loads(path.read_text())
   if valid_report(role,report,request,release) and any(parent_chain_matches(report.get('parent_pid'),prefix) for prefix in allowed):
    reports[role]=report;break
 if not validate_reports(reports,request,release):raise RuntimeError('all real-launch proofs and app parents must pass for this build')
 return reports
def check_real_proofs(base,release):
 request=json.loads((base/'verification.json').read_text())
 if not controls_alive(request):raise RuntimeError('synthetic controls expired; rerun verification')
 return select_real_proofs(base,request,release)

def start_verification(base,release):
 endpoint=base/'s'
 if endpoint.exists():
  with socket.socket(socket.AF_UNIX) as connection:
   try:connection.connect(str(endpoint))
   except ConnectionRefusedError:pass
   else:raise RuntimeError('stop active bridge before verification')
 witness=ROOT/'work/real-launch-witness';witness.write_bytes(b'synthetic-isolation-witness');witness.chmod(0o600)
 request={'nonce':secrets.token_hex(32),'witness':str(witness),'binary_sha256':release['sha256'],'extension_id':release['extension_id'],'tcp4':'','tcp6':''}
 seed=ROOT/'work/verification-seed.json';seed.write_bytes(encode(request));seed.chmod(0o600)
 process=subprocess.Popen([sys.executable,str(ROOT/'scripts/live-controls.py'),str(seed),str(base/'verification.json')],stdin=subprocess.DEVNULL,stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL,start_new_session=True)
 for _ in range(100):
  path=base/'verification.json'
  if path.exists() and json.loads(path.read_text()).get('nonce')==request['nonce']:return
  if process.poll() is not None:raise RuntimeError('verification controls failed')
  time.sleep(0.02)
 process.terminate();raise RuntimeError('verification controls startup timed out')

def main():
 parser=argparse.ArgumentParser();mode=parser.add_mutually_exclusive_group(required=True);mode.add_argument('--dry-run',action='store_true');mode.add_argument('--apply',action='store_true');parser.add_argument('--stage',choices=['bootstrap','activate'],default='bootstrap');args=parser.parse_args()
 release=metadata();exe=release.get('installation_target',release['executable']);home=Path.home();base=home/'Library/Containers/dev.david.brave-isole/Data/b'
 latest=ROOT/'work/install-receipt.json';previous=json.loads(latest.read_text()) if latest.exists() else []
 owned={item['path']:item['entry'] for item in previous}
 role_mode='verify' if args.stage=='bootstrap' else 'mcp'
 codex=home/'.codex/config.toml';claude=home/'Library/Application Support/Claude/claude_desktop_config.json'
 changes=[plan_toml(codex,{'command':exe,'args':['--'+role_mode+'-codex'],'enabled':True},owned.get(str(codex))),plan_json(claude,{'command':exe,'args':['--'+role_mode+'-claude']},owned.get(str(claude)))]
 host=home/'Library/Application Support/Google/Chrome/NativeMessagingHosts/dev.david.brave_isole.json'
 host_bytes=encode(host_manifest(Path(exe),release['extension_id']));old=host.read_bytes() if host.exists() else b''
 if old and old!=host_bytes:raise RuntimeError('native host manifest conflict')
 if not old:changes.append(change(host,old,host_bytes,'native',{}))
 changes=[item for item in changes if item is not None]
 summary={'stage':args.stage,'changes':[{'path':str(c['path']),'before_hash':c['hash'],'after_hash':digest(c['after']),'kind':c['kind']} for c in changes]}
 plan_path=ROOT/'work/install-plan.json'
 if args.dry_run:
  plan_path.write_bytes(encode(summary));plan_path.chmod(0o600);print(json.dumps({**summary,'status':'dry-run','extension_id':release['extension_id'],'browser_commands_enabled':args.stage=='activate','missing_clients':[str(p) for p in [codex,claude] if not p.exists()]},indent=2));return
 if not plan_path.exists() or json.loads(plan_path.read_text())!=summary:raise RuntimeError('dry-run stale or missing')
 proof=json.loads((ROOT/'work/isolation-release.json').read_text())
 if proof.get('verdict')!='PASS_CANDIDATE' or proof.get('executable_sha256')!=release['sha256']:raise RuntimeError('isolation proof missing for this executable')
 # The activation gate runs before any client entry can change to browser mode.
 if args.stage=='activate':real_proofs=check_real_proofs(base,release)
 target=Path(exe);source_app=Path(release['executable']).parents[2];target_app=target.parents[2]
 if not target_app.exists():
  target_app.parent.mkdir(parents=True,mode=0o700,exist_ok=True);shutil.copytree(source_app,target_app)
 if not target.is_file() or digest(target.read_bytes())!=release['sha256']:raise RuntimeError('installed app differs; left intact')
 subprocess.run(['/usr/bin/codesign','--verify','--strict',str(target_app)],check=True,capture_output=True)
 if args.stage=='bootstrap':
  if not (base/'verification.json').exists():start_verification(base,release)
  else:
   request=json.loads((base/'verification.json').read_text())
   if request.get('binary_sha256')!=release['sha256']:raise RuntimeError('verification belongs to another build; review first')
   if not controls_alive(request):start_verification(base,release)
 for item in changes:
  if item['kind']=='native':item['path'].parent.mkdir(parents=True,exist_ok=True)
 backup=ROOT/'work/private-backups'/datetime.now(timezone.utc).strftime('%Y%m%dT%H%M%S.%fZ')
 receipt=apply_changes(changes,backup);combined=merge_receipts(previous,receipt)
 if combined:atomic_write(latest,encode(combined),digest(latest.read_bytes() if latest.exists() else b''))
 if args.stage=='activate':
  accepted=ROOT/'work/real-launch-proofs.json';accepted.write_bytes(encode(real_proofs));accepted.chmod(0o600)
  (base/'verification.json').unlink()
 print(json.dumps({'status':'applied','stage':args.stage,'changed_files':len(receipt),'browser_commands_enabled':args.stage=='activate','backup':str(backup)},indent=2))
if __name__=='__main__':
 try:main()
 except (OSError,ValueError,RuntimeError):raise SystemExit('Configuration requires review; activation was not verified or installation could not be completed.')
