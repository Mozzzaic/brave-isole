#!/usr/bin/env python3
"""Run synthetic probes. A crash or unreachable endpoint is never a sandbox PASS."""
import argparse
import ctypes
import hashlib
import json
import os
from pathlib import Path
import socket
import subprocess
import tempfile

DENIED_CHECKS = ('outside_read', 'outside_write', 'tcp4', 'tcp6', 'udp4', 'udp6', 'external_tcp', 'child_read')
BUNDLE_ID = 'dev.david.brave-isole.probe'
ROOT = Path(__file__).resolve().parents[1]


def classify(results):
    if any(results.get(key, {}).get('ok') is True for key in DENIED_CHECKS):
        return 'FAIL_NOT_CONFINED'
    for key in DENIED_CHECKS:
        result = results.get(key, {})
        if result.get('ok') is not False or result.get('errno') not in (1, 13):
            return 'STOP_INCONCLUSIVE'
    if not all(results.get(key, {}).get('ok') is True for key in ('container_write', 'container_read', 'private_ipc', 'confined_peer_ipc')):
        return 'STOP_INCONCLUSIVE'
    return 'PASS_CANDIDATE'


def command(executable,*args):
    prefix=['--diagnose-isolation'] if executable.name=='brave-isole' else []
    return [str(executable),*prefix,*map(str,args)]


def probe(executable, *args):
    try:
        process = subprocess.run(command(executable,*args), cwd='/', capture_output=True, timeout=6)
    except (OSError, subprocess.TimeoutExpired) as error:
        return {'launch_error': type(error).__name__}
    if process.returncode:
        return {'returncode': process.returncode}
    try:
        result = json.loads(process.stdout)
        if set(result) != {'ok', 'errno'} or type(result['ok']) is not bool or type(result['errno']) is not int:
            return {'invalid_result': True}
        return result
    except (ValueError, TypeError):
        return {'invalid_result': True}


def run(candidate):
    bare = ROOT / 'target/debug/brave-isole-probe'
    executable = bare if candidate == 'probe' else ROOT / ('work/BraveIsole.app/Contents/MacOS/brave-isole' if candidate=='release' else 'work/BraveIsoleProbe.app/Contents/MacOS/brave-isole-probe')
    results, controls = {}, {}
    with tempfile.TemporaryDirectory(prefix='isolation-', dir=ROOT / 'work') as tmp:
        witness = Path(tmp) / 'synthetic-witness'
        witness.write_bytes(b'brave-isole-synthetic-witness')
        witness.chmod(0o600)
        results['outside_read'] = probe(executable, 'read', witness)
        results['outside_write'] = probe(executable, 'write', witness)
        results['child_read'] = probe(executable, 'child', 'read', witness)
        for version, family, address in [('4', socket.AF_INET, '127.0.0.1'), ('6', socket.AF_INET6, '::1')]:
            for transport, kind in [('tcp', socket.SOCK_STREAM), ('udp', socket.SOCK_DGRAM)]:
                key = transport + version
                try:
                    with socket.socket(family, kind) as server:
                        server.bind((address, 0))
                        if kind == socket.SOCK_STREAM:
                            server.listen(8)
                        port = server.getsockname()[1]
                        target = f'{address}:{port}' if family == socket.AF_INET else f'[{address}]:{port}'
                        controls[key] = probe(bare, transport, target)
                        results[key] = probe(executable, transport, target)
                except OSError as error:
                    controls[key] = {'setup_errno': error.errno}
                    results[key] = {'setup_errno': error.errno}
        controls['external_tcp'] = probe(bare, 'tcp', '1.1.1.1:443')
        results['external_tcp'] = probe(executable, 'tcp', '1.1.1.1:443')
        container = Path.home() / 'Library/Containers' / ('dev.david.brave-isole' if candidate=='release' else BUNDLE_ID) / 'Data'
        # Only the OS is allowed to create the app container for this test.
        if container.is_dir():
            marker = container / f'p-{os.getpid()}'
            results['container_write'] = probe(executable, 'write', marker)
            results['container_read'] = probe(executable, 'read', marker)
            marker.unlink(missing_ok=True)
            ipc = container / f's{os.getpid()}'
            try:
                with socket.socket(socket.AF_UNIX, socket.SOCK_STREAM) as server:
                    server.bind(str(ipc))
                    ipc.chmod(0o600)
                    server.listen(8)
                    controls['private_ipc'] = probe(bare, 'unix-connect', ipc)
                    results['private_ipc'] = probe(executable, 'unix-connect', ipc)
            except OSError as error:
                results['private_ipc'] = {'setup_errno': error.errno}
            finally:
                ipc.unlink(missing_ok=True)
            peer = container / f't{os.getpid()}'
            process = subprocess.Popen(command(executable,'unix-serve',peer), cwd='/', stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
            try:
                ready = process.stdout.readline().strip()
                if ready == '{"ready":true}':
                    # Diagnostic only: private Apple API, not part of shipped runtime.
                    library = ctypes.CDLL('/usr/lib/libsandbox.1.dylib')
                    function = library.sandbox_check
                    function.argtypes = [ctypes.c_int, ctypes.c_char_p, ctypes.c_int]
                    function.restype = ctypes.c_int
                    results['runtime_diagnostic'] = {'candidate': function(process.pid, None, 0), 'unconfined_control': function(os.getpid(), None, 0)}
                    results['confined_peer_ipc'] = probe(executable, 'unix-connect', peer)
                else:
                    results['confined_peer_ipc'] = {'server_not_ready': True}
                process.communicate(timeout=5)
            finally:
                if process.poll() is None:
                    process.kill()
                    process.communicate()
                peer.unlink(missing_ok=True)
        else:
            results['container_write'] = {'container_absent': True}
            results['private_ipc'] = {'container_absent': True}
    verdict = classify(results)
    if verdict == 'PASS_CANDIDATE' and results.get('runtime_diagnostic') != {'candidate': 1, 'unconfined_control': 0}:
        verdict = 'STOP_INCONCLUSIVE'
    if verdict == 'PASS_CANDIDATE' and not all(controls.get(key, {}).get('ok') is True for key in ('tcp4', 'tcp6', 'udp4', 'udp6', 'external_tcp', 'private_ipc')):
        verdict = 'STOP_INCONCLUSIVE'
    return {'candidate': candidate, 'verdict': verdict, 'results': results, 'positive_controls': controls, 'real_clients_tested': False,'executable_sha256':hashlib.sha256(executable.read_bytes()).hexdigest()}


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--candidate', choices=['probe', 'packaged','release'], required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    report = run(args.candidate)
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(report, indent=2) + '\n')
    args.output.chmod(0o600)
    print(report['verdict'])
    return 0 if report['verdict'] == 'PASS_CANDIDATE' else 1


if __name__ == '__main__':
    raise SystemExit(main())
