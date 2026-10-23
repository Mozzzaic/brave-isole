#!/usr/bin/env python3
"""Exercise the signed executable in both real CLI roles, with synthetic browser replies."""
import ctypes,json,select,struct,subprocess
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]
def line(process):
 if not select.select([process.stdout],[],[],10)[0]:raise RuntimeError('MCP timed out')
 return json.loads(process.stdout.readline())
def send(process,message):process.stdin.write((json.dumps(message)+'\n').encode());process.stdin.flush()
def read_bytes(stream,length):
 data=b''
 while len(data)<length:
  if not select.select([stream],[],[],10)[0]:raise RuntimeError('native timed out')
  part=stream.read(length-len(data))
  if not part:raise RuntimeError('native disconnected')
  data+=part
 return data
def native_read(process):
 length=struct.unpack('=I',read_bytes(process.stdout,4))[0]
 if length>1_048_576:raise RuntimeError('oversize')
 return json.loads(read_bytes(process.stdout,length))
def native_send(process,frame):
 data=json.dumps(frame).encode();process.stdin.write(struct.pack('=I',len(data))+data);process.stdin.flush()
def main():
 release=json.loads((ROOT/'work/release.json').read_text());exe=release['executable'];children=[]
 try:
  host=subprocess.Popen([exe,f'chrome-extension://{release["extension_id"]}/'],stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.PIPE,bufsize=0,cwd='/');children.append(host)
  hello=native_read(host)
  if hello['payload']!={'type':'hello'}:raise RuntimeError('no native hello')
  clients=[]
  for role in ['codex','claude']:
   client=subprocess.Popen([exe,'--mcp-'+role],stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.PIPE,bufsize=0,cwd='/');children.append(client);clients.append((role,client))
   send(client,{'jsonrpc':'2.0','id':1,'method':'initialize','params':{'protocolVersion':'2025-03-26','capabilities':{},'clientInfo':{'name':'synthetic-local-test','version':'1'}}})
   if 'result' not in line(client):raise RuntimeError('MCP initialization failed')
   send(client,{'jsonrpc':'2.0','method':'notifications/initialized'})
   send(client,{'jsonrpc':'2.0','id':2,'method':'tools/list','params':{}})
   if len(line(client)['result']['tools'])!=11:raise RuntimeError('tool list mismatch')
   send(client,{'jsonrpc':'2.0','id':3,'method':'tools/call','params':{'name':'browser_tabs','arguments':{}}})
  requests=[native_read(host),native_read(host)]
  for frame in reversed(requests):
   role=frame['payload']['role'];native_send(host,{'version':1,'epoch':frame['epoch'],'id':frame['id'],'payload':{'type':'reply','reply':{'ok':True,'result':{'role':role,'tabs':[]},'error':None}}})
  for role,client in clients:
   result=line(client)['result'];payload=json.loads(result['content'][0]['text'])
   if payload!={'role':role,'tabs':[]}:raise RuntimeError('cross-role routing failure')
  library=ctypes.CDLL('/usr/lib/libsandbox.1.dylib');check=library.sandbox_check;check.argtypes=[ctypes.c_int,ctypes.c_char_p,ctypes.c_int];check.restype=ctypes.c_int
  if any(check(process.pid,None,0)!=1 for process in children):raise RuntimeError('role not confined')
  report={'signed_native_host':True,'signed_codex_mcp':True,'signed_claude_mcp':True,'tools_per_client':11,'reversed_replies_routed_correctly':True,'all_runtime_sandboxed':True,'real_browser_or_desktop_parent_tested':False}
  p=ROOT/'work/release-smoke.json';p.write_text(json.dumps(report,indent=2)+'\n');p.chmod(0o600);print(json.dumps(report,indent=2))
 finally:
  for process in reversed(children):
   try:process.stdin.close();process.wait(timeout=5)
   except (OSError,subprocess.TimeoutExpired):process.kill();process.wait()
if __name__=='__main__':main()
