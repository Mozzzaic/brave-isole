import test from 'node:test';
import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import vm from 'node:vm';
test('popup_updates_after_native_handshake_takes_longer_than_initial_refresh',async()=>{
 let state={connected:false,verificationPassed:true},now=0;
 const timers=[],listeners={},status={textContent:''};
 const document={querySelector:selector=>selector==='#status'?status:{addEventListener:(event,fn)=>{listeners[selector+event]=fn;}}};
 const schedule=(fn,delay,interval=0)=>{const timer={fn,due:now+delay,interval};timers.push(timer);return timer;};
 const advance=async milliseconds=>{const end=now+milliseconds;for(;;){timers.sort((a,b)=>a.due-b.due);if(!timers.length||timers[0].due>end)break;const timer=timers.shift();now=timer.due;await timer.fn();if(timer.interval){timer.due+=timer.interval;timers.push(timer);}}now=end;await Promise.resolve();};
 const chrome={runtime:{sendMessage:async message=>message.action==='status'?state:{connecting:true}}};
 vm.runInNewContext(await readFile(new URL('../popup.js',import.meta.url),'utf8'),{document,chrome,setTimeout:(fn,delay)=>schedule(fn,delay),setInterval:(fn,delay)=>schedule(fn,delay,delay)});
 await Promise.resolve();await listeners['#connectclick']();await advance(300);
 assert.match(status.textContent,/Vérification/);
 state={connected:true,verificationPassed:false};await advance(1500);
 assert.match(status.textContent,/Pont connecté/);
 state={connected:false,verificationPassed:false};await advance(1500);
 assert.equal(status.textContent,'Pont arrêté.');
});

test('popup_reports_when_a_manual_pause_cannot_be_saved',async()=>{const status={textContent:''};const document={querySelector:selector=>selector==='#status'?status:{addEventListener:()=>{}}};const chrome={runtime:{sendMessage:async()=>({connected:false,persistenceError:true})}};vm.runInNewContext(await readFile(new URL('../popup.js',import.meta.url),'utf8'),{document,chrome,setTimeout:()=>{},setInterval:()=>{}});await new Promise(resolve=>setImmediate(resolve));assert.match(status.textContent,/désactive l.extension/i);});
