import test from 'node:test';
import assert from 'node:assert/strict';
import {AutoConnection} from '../connection.js';

const key='autoConnectEnabled';
class LocalSettings {
 constructor(value){this.values=value===undefined?{}:{[key]:value};}
 async get(name){assert.equal(name,key);return {...this.values};}
 async set(value){assert.deepEqual(Object.keys(value),[key]);assert.equal(typeof value[key],'boolean');Object.assign(this.values,value);}
}
function instance(storage){let connections=0,stops=0;const policy=new AutoConnection({storage,connect:()=>{connections++;},stop:async()=>{stops++;}});return {policy,connections:()=>connections,stops:()=>stops};}
const gate=()=>{let resolve;return {promise:new Promise(r=>{resolve=r;}),resolve:value=>resolve(value)};};

test('startup_connects_without_a_popup_and_reconnects_in_a_new_worker',async()=>{
 const settings=new LocalSettings();const first=instance(settings);await first.policy.restore();assert.equal(first.connections(),1);const restarted=instance(settings);await restarted.policy.restore();assert.equal(restarted.connections(),1);
});
test('manual_stop_persists_across_browser_and_worker_restarts',async()=>{
 const settings=new LocalSettings();const first=instance(settings);await first.policy.restore();assert.deepEqual(await first.policy.pause(),{stopped:true,persisted:true});assert.equal(first.stops(),1);const restarted=instance(settings);await restarted.policy.restore();assert.equal(restarted.connections(),0);assert.equal(restarted.policy.enabled,false);
});
test('manual_connect_restores_automatic_connection_after_a_stop',async()=>{
 const settings=new LocalSettings(false);const first=instance(settings);await first.policy.restore();await first.policy.enable();assert.equal(first.connections(),1);const restarted=instance(settings);await restarted.policy.restore();assert.equal(restarted.connections(),1);
});
test('manual_stop_wins_over_a_pending_startup_read',async()=>{
 const delayed=gate();const settings=new LocalSettings();settings.get=()=>delayed.promise;const first=instance(settings);const reading=first.policy.restore();await first.policy.pause();delayed.resolve({[key]:true});await reading;assert.equal(first.connections(),0);assert.equal(first.policy.enabled,false);
});
test('stop_wins_over_a_slow_manual_connect_write',async()=>{
 const delayed=gate(),started=gate();const settings=new LocalSettings(false);const write=settings.set.bind(settings);let once=true;settings.set=async value=>{if(once){once=false;started.resolve();await delayed.promise;}await write(value);};
 const first=instance(settings);const connecting=first.policy.enable();await started.promise;const stopping=first.policy.pause();assert.equal(first.stops(),1);delayed.resolve();await Promise.all([connecting,stopping]);assert.equal(first.connections(),0);const restarted=instance(settings);await restarted.policy.restore();assert.equal(restarted.connections(),0);
});
test('unreadable_or_invalid_settings_do_not_enable_connection',async()=>{
 const broken=instance({get:async()=>{throw new Error('storage unavailable');}});await broken.policy.restore();assert.equal(broken.connections(),0);assert.equal(broken.policy.persistenceError,true);
 const malformed=instance(new LocalSettings('true'));await malformed.policy.restore();assert.equal(malformed.connections(),0);
});
test('a_failed_stop_save_still_stops_immediately_and_is_reported',async()=>{
 const settings=new LocalSettings(true);const first=instance(settings);await first.policy.restore();settings.set=async()=>{throw new Error('disk unavailable');};assert.deepEqual(await first.policy.pause(),{stopped:true,persisted:false});assert.equal(first.stops(),1);assert.equal(first.policy.enabled,false);assert.equal(first.policy.persistenceError,true);await first.policy.restore();assert.equal(first.connections(),1);
});
test('startup_events_are_idempotent_inside_one_worker',async()=>{
 const first=instance(new LocalSettings());await Promise.all([first.policy.restore(),first.policy.restore()]);assert.equal(first.connections(),1);
});
