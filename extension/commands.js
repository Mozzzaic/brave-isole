import {checkedUrl,checkCommand,fail} from './policy.js';
const MAX_TEXT=262_144;
export class BrowserCommands {
  #api;#ownership;#queues=new Map();#groups=new Map();
  constructor(api,ownership){this.#api=api;this.#ownership=ownership;}
  async #target(session,handle){if(session.deadline!==undefined&&performance.now()>=session.deadline)fail('timeout');const item=this.#ownership.authorizeTab(session,handle);const tab=await this.#api.tabs.get(item.tabId);if(!tab||!this.#ownership.noteUrl(item.tabId,tab.url||''))fail('forbidden_url');this.#ownership.authorizeTab(session,handle);if(session.deadline!==undefined&&performance.now()>=session.deadline)fail('timeout');return {tabId:item.tabId};}
  async #send(session,handle,method,params={}){const target=await this.#target(session,handle);return this.#api.debugger.sendCommand(target,method,params);}
  async #evaluate(session,handle,expression){
    const tree=await this.#send(session,handle,'Page.getFrameTree');
    const world=await this.#send(session,handle,'Page.createIsolatedWorld',{frameId:tree.frameTree.frame.id,worldName:'brave-isole'});
    const value=await this.#send(session,handle,'Runtime.evaluate',{expression,contextId:world.executionContextId,awaitPromise:true,returnByValue:true,timeout:28000});
    if(value.exceptionDetails)fail('invalid_request');
    const result=value.result?.value??null;
    if(new TextEncoder().encode(JSON.stringify(result)).length>MAX_TEXT)fail('too_large');return result;
  }
  handleCommand(session,input,expiresAt=Date.now()+30_000){
    session={...session,deadline:performance.now()+Math.max(0,Math.min(30_000,expiresAt-Date.now()))};
    checkCommand(input);this.#ownership.session(session);
    const key=input.tab||`${session.epoch}:${session.role}:open`;
    const previous=this.#queues.get(key)||Promise.resolve();
    const next=previous.catch(()=>{}).then(()=>this.#execute(session,input));
    this.#queues.set(key,next);next.finally(()=>{if(this.#queues.get(key)===next)this.#queues.delete(key);}).catch(()=>{});
    return next;
  }
  async #execute(session,c){
    this.#ownership.session(session);if(performance.now()>=session.deadline)fail('timeout');
    if(c.op==='open'){
      const url=checkedUrl(c.url);const tab=await this.#api.tabs.create({url:'about:blank',active:false});
      if(!Number.isInteger(tab.id))fail('unavailable');
      const handle=this.#ownership.add(session.role,tab.id);
      try{
        await this.#api.debugger.attach({tabId:tab.id},'1.3');
        const groupKey=`${session.epoch}:${session.role}`;let groupId=this.#groups.get(groupKey);
        try{groupId=await this.#api.tabs.group({tabIds:[tab.id],...(groupId===undefined?{}:{groupId})});}
        catch{groupId=await this.#api.tabs.group({tabIds:[tab.id]});}
        this.#groups.set(groupKey,groupId);
        await this.#api.tabGroups.update(groupId,{title:session.role==='codex'?'Codex':'Claude',color:session.role==='codex'?'blue':'orange'});
        await this.#send(session,handle,'Page.enable');
        const result=await this.#send(session,handle,'Page.navigate',{url});
        if(result.errorText)fail('unavailable');
        this.#ownership.authorizeTab(session,handle);return {tab:handle};
      }catch(error){this.#ownership.forget(tab.id);try{await this.#api.debugger.detach({tabId:tab.id});}catch{}throw error;}
    }
    if(c.op==='list'){
      const values=[];
      for(const item of this.#ownership.list(session)){
        try{const target=await this.#target(session,item.handle);const tab=await this.#api.tabs.get(target.tabId);values.push({tab:item.handle,url:tab.url,title:tab.title||''});}catch{}
      }return {tabs:values};
    }
    const target=await this.#target(session,c.tab);
    if(c.op==='close'){this.#ownership.forget(target.tabId);try{await this.#api.debugger.detach(target);}catch{}await this.#api.tabs.remove(target.tabId);return {closed:true};}
    if(c.op==='navigate'){const response=await this.#send(session,c.tab,'Page.navigate',{url:checkedUrl(c.url)});if(response.errorText)fail('unavailable');return {navigating:true};}
    if(c.op==='evaluate')return this.#evaluate(session,c.tab,c.expression);
    if(c.op==='snapshot'){
      const value=await this.#send(session,c.tab,'Accessibility.getFullAXTree');
      const nodes=[];let bytes=0,truncated=false;
      for(const node of value.nodes||[]){if(node.ignored)continue;const row={role:node.role?.value||'',name:node.name?.value||'',...(node.value?{value:node.value.value}:{})};const size=new TextEncoder().encode(JSON.stringify(row)).length;if(bytes+size>MAX_TEXT-2048){truncated=true;break;}bytes+=size;nodes.push(row);}
      return {nodes,truncated};
    }
    if(c.op==='screenshot'){
      for(const quality of [60,40,20]){const value=await this.#send(session,c.tab,'Page.captureScreenshot',{format:'jpeg',quality,fromSurface:true,captureBeyondViewport:false});if(typeof value.data==='string'&&value.data.length<900_000)return {mimeType:'image/jpeg',data:value.data};}fail('too_large');
    }
    if(c.op==='scroll')return this.#evaluate(session,c.tab,`(()=>{window.scrollBy(${c.x},${c.y});return true})()`);
    const query=`const nodes=document.querySelectorAll(${JSON.stringify(c.selector)});if(nodes.length!==1)throw new Error('selector');const el=nodes[0];`;
    if(c.op==='click')return this.#evaluate(session,c.tab,`(()=>{${query}el.click();return true})()`);
    if(c.op==='fill')return this.#evaluate(session,c.tab,`(()=>{${query}const value=${JSON.stringify(c.value)};if(el instanceof HTMLInputElement||el instanceof HTMLTextAreaElement){const p=el instanceof HTMLInputElement?HTMLInputElement.prototype:HTMLTextAreaElement.prototype;Object.getOwnPropertyDescriptor(p,'value').set.call(el,value)}else if(el.isContentEditable){el.textContent=value}else{throw new Error('element')}el.dispatchEvent(new Event('input',{bubbles:true}));el.dispatchEvent(new Event('change',{bubbles:true}));return true})()`);
    if(c.op==='wait')return this.#evaluate(session,c.tab,`new Promise((resolve,reject)=>{const start=Date.now();const timer=setInterval(()=>{if(document.querySelector(${JSON.stringify(c.selector)})){clearInterval(timer);resolve(true)}else if(Date.now()-start>25000){clearInterval(timer);reject(new Error('timeout'))}},100)})`);
    fail('invalid_request');
  }
}
