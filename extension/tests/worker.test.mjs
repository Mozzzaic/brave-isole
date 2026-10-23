import test from 'node:test';import assert from 'node:assert/strict';
const events={};const event=name=>({addListener:fn=>{events[name]=fn;}});const detached=[];
globalThis.chrome={tabs:{onRemoved:event('removed'),onUpdated:event('updated')},debugger:{onDetach:event('detach'),onEvent:event('cdp'),detach:async target=>detached.push(target)},storage:{local:{get:async()=>({autoConnectEnabled:false}),set:async()=>{}}},runtime:{onStartup:event('startup'),onInstalled:event('installed'),onMessage:event('message'),id:'self',getURL:p=>`chrome-extension://self/${p}`}};
await import('../worker.js');
test('personal_tab_navigation_never_sends_a_debugger_command',async()=>{events.updated(999,{url:'https://example.org/'});await new Promise(r=>setImmediate(r));assert.deepEqual(detached,[]);});
