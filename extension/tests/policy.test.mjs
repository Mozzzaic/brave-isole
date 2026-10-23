import test from 'node:test';
import assert from 'node:assert/strict';
import {Ownership, checkedUrl, checkCommand} from '../policy.js';
const session={epoch:'epoch',role:'codex'};
function registry(){const r=new Ownership();r.start('epoch');return r;}
test('foreign_handle_never_attaches',()=>{const r=registry();const h=r.add('claude',1);assert.throws(()=>r.authorizeTab(session,h),/unauthorized/);});
test('list_filters_foreign_tabs',()=>{const r=registry();r.add('codex',1);r.add('claude',2);assert.equal(r.list(session).length,1);assert.equal(r.list(session)[0].tabId,1);});
test('forbidden_schemes_rejected',()=>{for(const u of ['file:///tmp/test','chrome://settings','brave://settings','javascript:alert(1)','https://user:pass@site.test','relative','chrome-extension://id/page'])assert.throws(()=>checkedUrl(u));assert.throws(()=>checkedUrl('https://allowed.test@evil.test/'),/forbidden_url/);});
test('navigation_revokes_unsafe_target',()=>{const r=registry();const h=r.add('codex',1);r.noteUrl(1,'https://example.org/');r.noteUrl(1,'file:///tmp/test');assert.throws(()=>r.authorizeTab(session,h),/unauthorized/);});
test('child_tab_requires_verified_opener',()=>{const r=registry();r.add('codex',1);assert.equal(r.ownerOf(999),undefined);assert.equal(r.ownerOf(1).role,'codex');});
test('debugger_detach_invalidates_handles',()=>{const r=registry();const h=r.add('codex',1);r.forget(1);assert.throws(()=>r.authorizeTab(session,h));});
test('service_worker_restart_does_not_reattach_tabs',()=>{const first=registry();const h=first.add('codex',1);assert.throws(()=>registry().authorizeTab(session,h));});
test('frame_target_must_belong_to_owned_tab',()=>{assert.throws(()=>checkCommand({op:'snapshot',tab:'h',sessionId:'foreign'}),/invalid_request/);});
test('unknown_browser_and_host_commands_are_rejected',()=>{for(const op of ['shell','cdp','cookies','activate','download'])assert.throws(()=>checkCommand({op}),/invalid_request/);});
test('revoked_epoch_rejects_handles',()=>{const r=registry();const h=r.add('codex',1);r.stop();assert.throws(()=>r.authorizeTab(session,h),/stale_session/);});
