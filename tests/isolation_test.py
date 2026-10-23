import importlib.util
from pathlib import Path
import unittest

spec = importlib.util.spec_from_file_location('verify_isolation', Path(__file__).parents[1] / 'scripts/verify-isolation.py')
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)

class ClassificationTests(unittest.TestCase):
    def denied(self):
        return {key: {'ok': False, 'errno': 1} for key in module.DENIED_CHECKS}

    def test_unconfined_file_read_is_not_a_pass(self):
        results = self.denied()
        results['outside_read'] = {'ok': True, 'errno': 0}
        self.assertEqual(module.classify(results), 'FAIL_NOT_CONFINED')

    def test_crash_is_not_evidence_of_confinement(self):
        results = self.denied()
        results['outside_read'] = {'returncode': -9}
        self.assertEqual(module.classify(results), 'STOP_INCONCLUSIVE')

    def test_refused_tcp_connection_does_not_prove_policy_denial(self):
        results = self.denied()
        results.update({'container_write': {'ok': True, 'errno': 0}, 'private_ipc': {'ok': True, 'errno': 0}, 'container_read': {'ok': True, 'errno': 0}, 'confined_peer_ipc': {'ok': True, 'errno': 0}})
        results['tcp4'] = {'ok': False, 'errno': 61}
        self.assertEqual(module.classify(results), 'STOP_INCONCLUSIVE')

    def test_expected_denials_need_positive_container_and_ipc_controls(self):
        self.assertEqual(module.classify(self.denied()), 'STOP_INCONCLUSIVE')

    def test_complete_denial_and_positive_controls_pass(self):
        results = self.denied()
        results.update({'container_write': {'ok': True, 'errno': 0}, 'private_ipc': {'ok': True, 'errno': 0}, 'container_read': {'ok': True, 'errno': 0}, 'confined_peer_ipc': {'ok': True, 'errno': 0}})
        self.assertEqual(module.classify(results), 'PASS_CANDIDATE')

    def test_same_identity_peer_ipc_must_be_proven(self):
        results = self.denied()
        results.update({'container_write': {'ok': True, 'errno': 0}, 'container_read': {'ok': True, 'errno': 0}, 'private_ipc': {'ok': True, 'errno': 0}})
        self.assertEqual(module.classify(results), 'STOP_INCONCLUSIVE')

if __name__ == '__main__':
    unittest.main()
