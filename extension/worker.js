import {Ownership} from './policy.js';
import {BrowserCommands} from './commands.js';
import {AutoConnection} from './connection.js';
const ownership=new Ownership();
const commands=new BrowserCommands(chrome,ownership);
let port=null,epoch=null,verificationPassed=false;
const errors=new Set(['invalid_request','unauthorized','stale_session','forbidden_url','too_large','timeout','disconnected','unavailable']);
async function stopAll(){
  const oldPort=port;port=null;epoch=null;
  const tabs=ownership.stop();
  try{oldPort?.postMessage({version:1,epoch:oldPort.bridgeEpoch,id:crypto.randomUUID(),payload:{type:'stop'}});}catch{}
  oldPort?.disconnect();
  await Promise.allSettled(tabs.map(t=>chrome.debugger.detach({tabId:t.tabId})));
}
function connect(){
  if(port)return;
  verificationPassed=false;
  const next=chrome.runtime.connectNative('dev.david.brave_isole');port=next;
  next.onDisconnect.addListener(()=>{void chrome.runtime.lastError;if(port===next)void stopAll();});
  next.onMessage.addListener(async message=>{
    if(port!==next)return;
    if(!message||typeof message!=='object'||Object.keys(message).some(k=>!['epoch','expires_at_ms','id','payload','version'].includes(k))||message.version!==1||typeof message.epoch!=='string'||typeof message.id!=='string'||message.epoch.length>128||message.id.length>128){void stopAll();return;}
    const payload=message.payload;
    if(payload?.type==='verification'&&Object.keys(payload).length===2&&typeof payload.passed==='boolean'){verificationPassed=payload.passed;return;}
    if(payload?.type==='hello'&&Object.keys(payload).length===1&&epoch===null){epoch=message.epoch;next.bridgeEpoch=epoch;ownership.start(epoch);return;}
    if(payload?.type!=='command'||Object.keys(payload).sort().join(',')!=='command,role,type'||message.epoch!==epoch||!Number.isSafeInteger(message.expires_at_ms)){void stopAll();return;}
    const session={epoch,role:payload.role};
    let reply;
    try{
      const result=await commands.handleCommand(session,payload.command,message.expires_at_ms);
      ownership.session(session);
      reply={ok:true,result,error:null};
    }catch(error){reply={ok:false,result:null,error:errors.has(error.message)?error.message:'unavailable'};}
    if(port!==next||epoch!==message.epoch||!ownership.active)return;
    const response={version:1,epoch,id:message.id,payload:{type:'reply',reply}};
    if(new TextEncoder().encode(JSON.stringify(response)).length>1_048_576)response.payload.reply={ok:false,result:null,error:'too_large'};
    try{next.postMessage(response);}catch{void stopAll();}
  });
}
chrome.tabs.onRemoved.addListener(id=>ownership.forget(id));
chrome.tabs.onUpdated.addListener((id,change)=>{if(change.url&&ownership.ownerOf(id)&&!ownership.noteUrl(id,change.url)){if(ownership.ownerOf(id))ownership.forget(id);chrome.debugger.detach({tabId:id}).catch(()=>{});}});
chrome.debugger.onDetach.addListener(target=>ownership.forget(target.tabId));
chrome.debugger.onEvent.addListener((target,method,params)=>{
  if(method==='Page.frameNavigated'&&params.frame?.url&&!params.frame.parentId&&ownership.ownerOf(target.tabId)){
    if(!ownership.noteUrl(target.tabId,params.frame.url))chrome.debugger.detach({tabId:target.tabId}).catch(()=>{});
  }
});
const lifecycle=new AutoConnection({storage:chrome.storage.local,connect,stop:stopAll});
chrome.runtime.onStartup.addListener(()=>lifecycle.restore());
chrome.runtime.onInstalled.addListener(()=>lifecycle.restore());
chrome.runtime.onMessage.addListener((message,sender,reply)=>{
  if(sender.id!==chrome.runtime.id||sender.url!==chrome.runtime.getURL('popup.html'))return false;
  if(message?.action==='status'){reply({connected:ownership.active,verificationPassed,autoConnectEnabled:lifecycle.enabled,persistenceError:lifecycle.persistenceError});return false;}
  if(message?.action==='connect'){lifecycle.enable().then(()=>reply({connecting:true})).catch(()=>reply({error:'preference_unavailable'}));return true;}
  if(message?.action==='stop'){lifecycle.pause().then(reply);return true;}
  return false;
});
void lifecycle.restore();
