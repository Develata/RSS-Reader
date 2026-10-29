"""Exercise the shared build identifier through Cargo, including rebuild tracking."""
import os
from pathlib import Path
import subprocess
import tempfile
import time
import unittest


ROOT = Path(__file__).resolve().parents[1]


class BuildIdTests(unittest.TestCase):
    def test_vendor_edits_invalidate_both_builds(self):
        scratch_parent = ROOT / 'target/test-build-id'
        scratch_parent.mkdir(parents=True, exist_ok=True)
        with tempfile.TemporaryDirectory(dir=scratch_parent) as scratch:
            root = Path(scratch)
            for directory in ['build-support', 'assets', 'icons', 'vendor/url/src', 'docs', 'tmp']:
                (root / directory).mkdir(parents=True)
            (root / 'build-support/build_id.rs').write_bytes(
                (ROOT / 'build-support/build_id.rs').read_bytes())
            (root / 'Cargo.toml').write_text(
                '[workspace]\nmembers = ["crates/page", "crates/server"]\nresolver = "3"\n')
            (root / 'Dioxus.toml').write_text('# fixture\n')
            (root / '.gitignore').write_text('/target/\n/tmp/\n')
            vendor = root / 'vendor/url/src/parser.rs'
            vendor.write_text('// original dependency\n')
            for package in ['page', 'server']:
                crate = root / 'crates' / package
                (crate / 'src').mkdir(parents=True)
                (crate / 'Cargo.toml').write_text(
                    f'[package]\nname = "{package}"\nversion = "0.1.0"\nedition = "2024"\n')
                (crate / 'build.rs').write_text(
                    'include!("../../build-support/build_id.rs");\n'
                    'fn main() { emit_build_id(); }\n')
                (crate / 'src/main.rs').write_text(
                    'fn main() { println!("{}", env!("RSSR_BUILD_ID")); }\n')

            env = os.environ.copy()
            for name in ['RSSR_BUILD_ID', 'GIT_DIR', 'GIT_WORK_TREE', 'GIT_INDEX_FILE']:
                env.pop(name, None)
            env.update(CARGO_TARGET_DIR=str(root / 'target'), TEMP=str(root / 'tmp'),
                       TMP=str(root / 'tmp'), TMPDIR=str(root / 'tmp'), CARGO_INCREMENTAL='0')

            def run(*args):
                return subprocess.run(args, cwd=root, env=env, check=True, timeout=90,
                                      text=True, capture_output=True).stdout.strip()

            run('git', 'init', '-q')
            run('git', 'config', 'core.autocrlf', 'false')
            run('git', 'config', 'core.hooksPath', str(root / 'tmp'))
            run('cargo', 'generate-lockfile', '--offline')
            run('git', 'add', '.')
            run('git', '-c', 'user.name=Build ID fixture', '-c', 'user.email=fixture@example.invalid',
                '-c', 'commit.gpgsign=false', 'commit', '-qm', 'fixture')

            def identifier():
                page = run('cargo', 'run', '-q', '--offline', '--locked', '-p', 'page')
                server = run('cargo', 'run', '-q', '--offline', '--locked', '-p', 'server')
                self.assertEqual(page, server, 'page and server must compute the same identifier')
                return page

            clean = identifier()
            self.assertNotIn('-dirty-', clean)
            # Avoid timestamp granularity masking Cargo's file watching on some filesystems.
            time.sleep(1.1)
            vendor.write_text('// patched dependency\n')
            changed = identifier()
            self.assertNotEqual(clean, changed, 'vendor-only changes must invalidate Cargo builds')
            self.assertIn('-dirty-', changed)
            time.sleep(1.1)
            vendor.write_text('// a different patch\n')
            second = identifier()
            self.assertNotEqual(changed, second, 'different vendor contents need different ids')
            time.sleep(1.1)
            extra = root / 'vendor/url/src/new.rs'
            extra.write_text('// untracked dependency source\n')
            self.assertNotEqual(second, identifier(), 'untracked vendor sources enter the hash')
            time.sleep(1.1)
            extra.unlink()
            vendor.write_text('// original dependency\n')
            self.assertEqual(clean, identifier(), 'reverting vendor edits restores the clean id')
            (root / 'docs/note.md').write_text('Documentation does not change the product.\n')
            self.assertEqual(clean, identifier())


if __name__ == '__main__':
    unittest.main()
