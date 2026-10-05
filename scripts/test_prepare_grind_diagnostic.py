#!/usr/bin/env python3
"""The isolated build must not silently change the pinned hash implementation."""
from contextlib import ExitStack, redirect_stdout
import io
import json
from pathlib import Path
import tarfile
from types import SimpleNamespace
import tempfile
import unittest
from unittest.mock import patch

import prepare_grind_diagnostic as driver

from prepare_grind_diagnostic import check_packages


class PackageIdentityTests(unittest.TestCase):
    def test_pruning_is_allowed_but_version_source_and_checksum_changes_are_not(self):
        with tempfile.TemporaryDirectory() as temporary:
            root, copy = Path(temporary) / 'root.lock', Path(temporary) / 'copy.lock'
            package = '[[package]]\nname = "blake3"\nversion = "1.8.7"\nsource = "registry+fixture"\nchecksum = "abc"\n'
            root.write_text(package + '\n[[package]]\nname = "unused"\nversion = "1.0.0"\n')
            copy.write_text(package)
            check_packages(root, copy)
            for before, after in (('1.8.7', '1.8.8'), ('registry+fixture', 'git+fixture'), ('abc', 'def')):
                with self.subTest(changed=before):
                    copy.write_text(package.replace(before, after))
                    with self.assertRaisesRegex(ValueError, 'absent from root'):
                        check_packages(root, copy)


class PreparationTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.root = Path(self.temp.name).resolve()
        self.files = {"Cargo.lock": '[[package]]\nname = "blake3"\nversion = "1.8.7"\n',
                      "rust-toolchain.toml": '[toolchain]\nchannel = "fixture"\n',
                      "vendor/flock-mod/Cargo.toml": '[workspace]\n',
                      "vendor/flock-mod/Cargo.lock": 'stale vendor lock\n',
                      "vendor/flock-mod/crates/flock-core/src/lib.rs": '// committed fixture\n',
                      "vendor/field/src/lib.rs": '// committed field fixture\n'}
        for name, contents in self.files.items():
            file = self.root / name
            file.parent.mkdir(parents=True, exist_ok=True)
            file.write_text(contents)
        self.args = SimpleNamespace(repo=self.root, revision="1" * 40, output=Path('.tmp/grind'), build_timeout=10)
        self.stack = ExitStack()
        self.stack.enter_context(redirect_stdout(io.StringIO()))
        self.verify = self.stack.enter_context(patch.object(driver, 'verify_revision'))
        self.stack.enter_context(patch.object(driver, 'require_ignored'))
        self.stack.enter_context(patch.object(driver, 'compiler_version', return_value='rustc fixture\n'))
        self.stack.enter_context(patch.object(driver.support, 'run_process', side_effect=self.archive))
        self.execute = self.stack.enter_context(patch.object(driver, 'execute', side_effect=self.simulate))

    def tearDown(self):
        self.stack.close()
        self.temp.cleanup()

    def archive(self, command, *, cwd, stdout, timeout):
        self.assertEqual(command[:4], ['git', 'archive', '--format=tar', self.args.revision])
        with tarfile.open(fileobj=stdout, mode='w') as archive:
            for name in self.files:
                archive.add(self.root / name, arcname=name)
        return 0, False

    def simulate(self, command, *, root, env, output, errors=None, timeout):
        if command[0] == 'tar':
            with tarfile.open(command[2]) as archive:
                archive.extractall(command[4], filter='data')
        else:
            self.assertEqual(command[:3], ['cargo', 'test', '--offline'])
            self.assertIn('--no-run', command)
            self.assertEqual(env['CARGO_PROFILE_RELEASE_LTO'], 'true')
            self.assertEqual(env['CARGO_PROFILE_RELEASE_CODEGEN_UNITS'], '1')
            self.assertEqual(env['RUSTFLAGS'], '-C target-cpu=native')
            binary = Path(env['CARGO_TARGET_DIR']) / 'test-binary'
            binary.parent.mkdir(parents=True, exist_ok=True)
            binary.write_text('test executable fixture')
            output.write_text(json.dumps({'reason': 'compiler-artifact', 'target': {'name': 'flock_core'}, 'executable': str(binary)}) + '\n')
        return 0.1

    def test_preparation_preserves_source_and_emits_only_manual_run_commands(self):
        path = driver.prepare(self.args)
        manifest = json.loads(path.read_text())
        self.assertEqual(manifest['status'], 'prepared')
        self.assertTrue(manifest['package_identities_preserved'])
        self.assertEqual(self.execute.call_count, 2)
        self.assertEqual(self.verify.call_count, 2)
        for name, contents in self.files.items():
            self.assertEqual((self.root / name).read_text(), contents)
        copied = path.parent / 'source/vendor/flock-mod/Cargo.lock'
        self.assertEqual(copied.read_text(), self.files['Cargo.lock'])
        self.assertEqual(len(manifest['diagnostic_commands']), 2)
        self.assertTrue(all(command[-3:] == ['--ignored', '--nocapture', '--test-threads=1'] for command in manifest['diagnostic_commands']))
        with self.assertRaisesRegex(ValueError, 'already exists'):
            driver.prepare(self.args)

    def test_failed_build_is_retained_without_a_runnable_manifest(self):
        def fail(command, **kwargs):
            if command[0] == 'cargo':
                raise RuntimeError('build failed')
            return self.simulate(command, **kwargs)
        self.execute.side_effect = fail
        with self.assertRaisesRegex(RuntimeError, 'build failed'):
            driver.prepare(self.args)
        manifest = json.loads((self.root / self.args.output / 'manifest.json').read_text())
        self.assertEqual(manifest['status'], 'failed')
        self.assertNotIn('diagnostic_commands', manifest)


if __name__ == '__main__':
    unittest.main()
