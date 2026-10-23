import importlib.util
from pathlib import Path
import unittest,tempfile,json,tomllib,os
from unittest.mock import patch
spec=importlib.util.spec_from_file_location('configure_clients',Path(__file__).parents[1]/'scripts/configure_clients.py');module=importlib.util.module_from_spec(spec);spec.loader.exec_module(module)
ENTRY={'command':'/test/BraveIsole.app/Contents/MacOS/brave-isole','args':['--mcp-claude']}
class InstallationTests(unittest.TestCase):
 def setUp(self):self.tmp=tempfile.TemporaryDirectory();self.root=Path(self.tmp.name)
 def tearDown(self):self.tmp.cleanup()
 def test_other_mcp_servers_unchanged(self):
  p=self.root/'claude.json';original={'otherSetting':42,'mcpServers':{'other':{'command':'test','env':{'SECRET':'synthetic-secret'}}}};p.write_text(json.dumps(original));change=module.plan_json(p,ENTRY);module.apply_changes([change],self.root/'backup');after=json.loads(p.read_text());self.assertEqual(after['mcpServers'].pop('brave_isole'),ENTRY);self.assertEqual(after,original)
 def test_codex_other_settings_and_comments_preserved(self):
  p=self.root/'config.toml';text='# Keep this comment\nmodel="example"\n[mcp_servers.other]\ncommand="test"\n';p.write_text(text);entry={'command':'/test/app','args':['--mcp-codex'],'enabled':True};change=module.plan_toml(p,entry);module.apply_changes([change],self.root/'backup');after=tomllib.loads(p.read_text());self.assertEqual(after['mcp_servers'].pop('brave_isole'),entry);self.assertEqual(after,tomllib.loads(text));self.assertTrue(p.read_text().startswith(text))
 def test_concurrent_edit_aborts(self):
  p=self.root/'claude.json';p.write_text('{}');change=module.plan_json(p,ENTRY);p.write_text('{"newUserSetting":true}');
  with self.assertRaises(RuntimeError):module.apply_changes([change],self.root/'backup')
  self.assertEqual(p.read_text(),'{"newUserSetting":true}')
 def test_malformed_config_is_not_rewritten(self):
  p=self.root/'claude.json';p.write_text('{broken');
  with self.assertRaises(ValueError):module.plan_json(p,ENTRY)
  self.assertEqual(p.read_text(),'{broken')
 def test_rollback_preserves_later_user_edit(self):
  p=self.root/'claude.json';p.write_text('{}');module.apply_changes([module.plan_json(p,ENTRY)],self.root/'backup');value=json.loads(p.read_text());value['newUserSetting']=42;p.write_text(json.dumps(value));module.restore_owned(p,ENTRY,'json');self.assertEqual(json.loads(p.read_text()),{'mcpServers':{},'newUserSetting':42})
 def test_rollback_does_not_remove_modified_entry(self):
  p=self.root/'claude.json';p.write_text(json.dumps({'mcpServers':{'brave_isole':{'command':'changed-by-user'}}}));
  with self.assertRaises(RuntimeError):module.restore_owned(p,ENTRY,'json')
 def test_missing_client_is_reported(self):self.assertIsNone(module.plan_json(self.root/'absent.json',ENTRY))
 def test_uninstall_does_not_touch_brave_profile(self):
  profile=self.root/'Brave/Default';profile.mkdir(parents=True);cookie=profile/'Cookies';cookie.write_bytes(b'synthetic-session');p=self.root/'claude.json';p.write_text('{}');module.apply_changes([module.plan_json(p,ENTRY)],self.root/'backup');module.restore_owned(p,ENTRY,'json');self.assertEqual(cookie.read_bytes(),b'synthetic-session')
 def test_host_manifest_allows_exact_extension_id(self):
  value=module.host_manifest(Path('/test/app'),'a'*32);self.assertEqual(value['allowed_origins'],['chrome-extension://'+'a'*32+'/']);self.assertEqual(value['type'],'stdio')
 def test_private_backups(self):
  p=self.root/'claude.json';p.write_text('{}');backup=self.root/'backup';module.apply_changes([module.plan_json(p,ENTRY)],backup);self.assertEqual(os.stat(backup).st_mode&0o777,0o700);self.assertTrue(all(os.stat(f).st_mode&0o777==0o600 for f in backup.iterdir() if f.is_file()))
 def test_repeat_install_preserves_receipt(self):
  p=self.root/'claude.json';p.write_text('{}');backup=self.root/'backup';receipt=module.apply_changes([module.plan_json(p,ENTRY)],backup);module.apply_changes([],backup);self.assertEqual(json.loads((backup/'receipt.json').read_text()),receipt)
 def test_merging_receipts_keeps_earlier_client_and_original_backup(self):
  first={'path':'one','entry':{'args':['verify']},'before_hash':'original','backup':'first','after_hash':'verify','kind':'json'}
  replacement={**first,'entry':{'args':['active']},'before_hash':'verify','backup':'second','after_hash':'active'}
  other={**first,'path':'two'};merged=module.merge_receipts([first],[replacement,other]);self.assertEqual(len(merged),2);self.assertEqual(merged[0]['entry'],{'args':['active']});self.assertEqual(merged[0]['backup'],'first');self.assertEqual(merged[0]['before_hash'],'original')
 def test_activation_requires_all_real_launch_proofs(self):
  release={'sha256':'binary','extension_id':'a'*32};request={'nonce':'nonce','binary_sha256':'binary','extension_id':'a'*32}
  self.assertFalse(module.validate_reports({},request,release))
  reports={role:{'passed':True,'role':role,'nonce':'nonce','binary_sha256':'binary','extension_id':'a'*32,'parent_pid':42,'checks':{'read_denied':True,'write_denied':True,'tcp4_denied':True,'tcp6_denied':True,'udp4_denied':True,'udp6_denied':True,'container':True,'private_ipc':True}} for role in ['native','codex','claude']}
  self.assertTrue(module.validate_reports(reports,request,release));reports['claude']['binary_sha256']='other-build';self.assertFalse(module.validate_reports(reports,request,release))
 def test_transient_probe_does_not_replace_live_app_launch_evidence(self):
  release={'sha256':'binary','extension_id':'a'*32};request={'nonce':'nonce'}
  for role in ['native','codex','claude']:
   report={'passed':True,'role':role,'nonce':'nonce','binary_sha256':'binary','extension_id':'a'*32,'parent_pid':42,'checks':{key:True for key in module.REQUIRED_CHECKS}}
   (self.root/f'proof-{role}-100.json').write_text(json.dumps(report))
   (self.root/f'proof-{role}.json').write_text(json.dumps({**report,'parent_pid':999}))
  with patch.object(module,'parent_chain_matches',side_effect=lambda pid,prefix:pid==42):
   reports=module.select_real_proofs(self.root,request,release)
  self.assertEqual({r['parent_pid'] for r in reports.values()},{42})
if __name__=='__main__':unittest.main()
