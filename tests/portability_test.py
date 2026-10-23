import importlib.util
import json
from pathlib import Path
import sys
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import patch

spec = importlib.util.spec_from_file_location('portable_configure', Path(__file__).parents[1] / 'scripts/configure_clients.py')
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)


class PortableInstallationTests(unittest.TestCase):
    def test_official_codex_app_name_passes_the_real_launch_gate(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            release = {'sha256': 'build', 'extension_id': 'a' * 32}
            request = {'nonce': 'nonce'}
            parents = {11: '/Applications/Brave Browser.app', 22: '/Applications/Codex.app', 33: '/Applications/Claude.app'}
            for role, pid in [('native', 11), ('codex', 22), ('claude', 33)]:
                report = {'passed': True, 'role': role, 'nonce': 'nonce', 'binary_sha256': 'build', 'extension_id': 'a' * 32, 'parent_pid': pid, 'checks': {key: True for key in module.REQUIRED_CHECKS}}
                (root / f'proof-{role}-{pid}.json').write_text(json.dumps(report))
            with patch.object(module, 'parent_chain_matches', side_effect=lambda pid, prefix: parents.get(pid) == str(prefix)):
                try:
                    proofs = module.select_real_proofs(root, request, release)
                except RuntimeError:
                    proofs = {}
            self.assertEqual(set(proofs), {'native', 'codex', 'claude'})

    def test_verification_starts_without_a_homebrew_python_path(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            (root / 'work').mkdir()
            base = root / 'container'
            base.mkdir()
            release = {'sha256': 'build', 'extension_id': 'a' * 32}

            def synthetic_control_process(command, **kwargs):
                if command[0] != '/portable/python3':
                    raise FileNotFoundError('Python is not installed in the Homebrew location')
                seed = json.loads(Path(command[2]).read_text())
                Path(command[3]).write_text(json.dumps(seed))
                return SimpleNamespace(poll=lambda: None, terminate=lambda: None)

            with patch.object(module, 'ROOT', root), patch.object(sys, 'executable', '/portable/python3'), patch.object(module.subprocess, 'Popen', side_effect=synthetic_control_process):
                try:
                    module.start_verification(base, release)
                except FileNotFoundError:
                    pass
            self.assertTrue((base / 'verification.json').exists(), 'The verification control must start with the running Python interpreter')


if __name__ == '__main__':
    unittest.main()
