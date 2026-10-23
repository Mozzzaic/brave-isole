import json,subprocess,select,sys,base64,os
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]
release=json.loads((ROOT/'work/release.json').read_text())
clients={};counter=0

def rpc(process,method,params=None):
 global counter
 counter+=1
 req={'jsonrpc':'2.0','id':counter,'method':method}
 if params is not None:req['params']=params
 process.stdin.write((json.dumps(req)+'\n').encode());process.stdin.flush()
 while True:
  if not select.select([process.stdout],[],[],35)[0]:raise RuntimeError('response_timeout')
  line=process.stdout.readline()
  if not line:raise RuntimeError('server_disconnected')
  reply=json.loads(line)
  if reply.get('id')==counter:return reply

def call(role,name,args):
 reply=rpc(clients[role],'tools/call',{'name':name,'arguments':args})
 if 'error' in reply:return reply
 result=reply['result']
 if name=='browser_screenshot' and not result.get('isError'):
  item=result['content'][0];path=ROOT/f'work/live-{role}.jpg';path.write_bytes(base64.b64decode(item['data']));path.chmod(0o600)
  return {'screenshot':str(path),'mimeType':item['mimeType'],'bytes':path.stat().st_size}
 if result.get('isError'):return {'error':json.loads(result['content'][0]['text'])}
 return json.loads(result['content'][0]['text'])

try:
 for role in ['codex','claude']:
  p=subprocess.Popen([release['installation_target'],'--mcp-'+role],stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.PIPE,bufsize=0,cwd='/');clients[role]=p
  reply=rpc(p,'initialize',{'protocolVersion':'2025-03-26','capabilities':{},'clientInfo':{'name':'local-synthetic-acceptance','version':'1'}})
  if 'result' not in reply:raise RuntimeError('initialize_failed')
  p.stdin.write(b'{"jsonrpc":"2.0","method":"notifications/initialized"}\n');p.stdin.flush()
  tools=rpc(p,'tools/list',{})['result']['tools']
  print(json.dumps({'role':role,'tools':len(tools),'pid':p.pid}),flush=True)
 for line in sys.stdin:
  try:
   command=json.loads(line);role=command['role']
   if command.get('action')=='fixture':
    html=(ROOT/'tests/fixtures/browser-page.html').read_text()
    expression='(()=>{document.open();document.write('+json.dumps(html)+');document.close();document.title="Test Brave isolé '+role+'";let count=0;document.querySelector("#increment").onclick=()=>document.querySelector("#count").textContent=String(++count);document.querySelector("#message").oninput=e=>document.querySelector("#value").textContent=e.target.value;return {ready:true,origin:location.origin}})()'
    result=call(role,'browser_evaluate',{'tab':command['tab'],'expression':expression})
   else:result=call(role,command['name'],command.get('arguments',{}))
   print(json.dumps({'role':role,'result':result},ensure_ascii=False),flush=True)
  except Exception as error:print(json.dumps({'harness_error':str(error)}),flush=True)
finally:
 for p in clients.values():
  try:p.stdin.close();p.wait(timeout=3)
  except (OSError,subprocess.TimeoutExpired):p.terminate();p.wait()
