export const fail = code => { throw new Error(code); };
export function checkedUrl(value) {
  if (typeof value !== 'string' || value.length > 16_384) fail('invalid_request');
  let url; try { url = new URL(value); } catch { fail('forbidden_url'); }
  if (!['https:', 'http:'].includes(url.protocol) || url.username || url.password) fail('forbidden_url');
  return url.href;
}
const shapes = {open:['url'],list:[],close:['tab'],navigate:['tab','url'],snapshot:['tab'],screenshot:['tab'],click:['tab','selector'],fill:['tab','selector','value'],scroll:['tab','x','y'],wait:['tab','selector'],evaluate:['tab','expression']};
export function checkCommand(command) {
  if (!command || typeof command !== 'object' || Array.isArray(command) || !Object.hasOwn(shapes,command.op)) fail('invalid_request');
  const keys=['op',...shapes[command.op]];
  if (Object.keys(command).length!==keys.length || keys.some(k=>!Object.hasOwn(command,k))) fail('invalid_request');
  for (const key of shapes[command.op]) {
    const value=command[key];
    if (key==='x'||key==='y') {if(!Number.isInteger(value)||value < -2147483648||value > 2147483647) fail('invalid_request');}
    else if (typeof value!=='string'||new TextEncoder().encode(value).length>262_144) fail('invalid_request');
  }
  if(Object.hasOwn(command,'url')) checkedUrl(command.url);
  return command;
}
export class Ownership {
  #epoch; #active=false; #tabs=new Map();
  start(epoch){this.#tabs.clear();this.#epoch=epoch;this.#active=true;}
  get active(){return this.#active;}
  stop(){this.#active=false;const owned=[...this.#tabs.values()];this.#tabs.clear();return owned;}
  session(session){if(!this.#active||session.epoch!==this.#epoch)fail('stale_session');if(!['codex','claude'].includes(session.role))fail('unauthorized');}
  add(role,tabId){this.session({epoch:this.#epoch,role});const handle=crypto.randomUUID();this.#tabs.set(handle,{handle,tabId,role,epoch:this.#epoch,bootstrapping:true});return handle;}
  authorizeTab(session,handle){this.session(session);const item=this.#tabs.get(handle);if(!item||item.role!==session.role||item.epoch!==session.epoch)fail('unauthorized');return item;}
  list(session){this.session(session);return [...this.#tabs.values()].filter(t=>t.role===session.role);}
  ownerOf(tabId){return [...this.#tabs.values()].find(t=>t.tabId===tabId);}
  forget(tabId){for(const [h,t]of this.#tabs)if(t.tabId===tabId)this.#tabs.delete(h);}
  noteUrl(tabId,url){const item=this.ownerOf(tabId);if(!item)return false;if(url==='about:blank'&&item.bootstrapping)return true;try{checkedUrl(url);item.bootstrapping=false;return true;}catch{this.forget(tabId);return false;}}
}
