import io
import tarfile
import tempfile
import unittest
from pathlib import Path

from package_web import create, unpack, verify


class WebPackageTests(unittest.TestCase):
    def setUp(self):
        self.scratch = tempfile.TemporaryDirectory(prefix='web-package-contract-')
        self.addCleanup(self.scratch.cleanup)
        self.root = Path(self.scratch.name)
        self.binary = self.root / 'rssr-web'
        self.binary.write_bytes(b'fixture executable; runtime is checked separately')
        self.binary.chmod(0o755)
        self.public = self.root / 'public'
        self.public.mkdir()
        (self.public / 'index.html').write_text('<html>fixture</html>')
        (self.public / 'app.wasm').write_bytes(b'fixture WASM git-fixture')

    def test_zip_and_tar_preserve_inventory_and_detect_changed_bytes(self):
        for suffix in ['.zip', '.tar.gz']:
            archive = self.root / ('bundle' + suffix)
            create(self.binary, self.public, 'git-fixture', archive)
            destination = self.root / ('unpack' + suffix)
            unpack(archive, destination)
            self.assertEqual(verify(destination)['build_id'], 'git-fixture')
            (destination / 'public/index.html').write_text('changed')
            with self.assertRaisesRegex(ValueError, 'hash mismatch'):
                verify(destination)

    def test_unknown_or_mismatched_build_is_rejected(self):
        for identifier in ['', 'dev-unknown', 'not-the-page-build']:
            with self.assertRaises(ValueError):
                create(self.binary, self.public, identifier, self.root / 'bad.zip')

    def test_existing_output_is_never_overwritten(self):
        archive = self.root / 'existing.zip'
        archive.write_bytes(b'existing data')
        with self.assertRaises(FileExistsError):
            create(self.binary, self.public, 'git-fixture', archive)
        self.assertEqual(archive.read_bytes(), b'existing data')

    def test_archive_path_traversal_and_links_are_rejected(self):
        for index, (name, link) in enumerate([('../outside', False), ('/outside', False), ('link', True)]):
            archive = self.root / f'bad{index}.tar.gz'
            with tarfile.open(archive, 'w:gz') as output:
                entry = tarfile.TarInfo(name)
                if link:
                    entry.type = tarfile.SYMTYPE
                    entry.linkname = '../outside'
                    output.addfile(entry)
                else:
                    entry.size = 1
                    output.addfile(entry, io.BytesIO(b'x'))
            with self.assertRaises(ValueError):
                unpack(archive, self.root / f'bad{index}')
        self.assertFalse((self.root / 'outside').exists())


if __name__ == '__main__':
    unittest.main()
