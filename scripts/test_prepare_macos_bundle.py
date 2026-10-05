#!/usr/bin/env python3
"""Portable fixture checks; these do not build or launch the macOS app."""

from pathlib import Path
import plistlib
import subprocess
import sys
import tarfile
import tempfile
import unittest

from prepare_macos_bundle import VERSION_KEYS, prepare_bundle


class MacOSBundleMetadataTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.bundle = self.root / "bundle with spaces" / "RSS-Reader.app"
        self.contents = self.bundle / "Contents"
        self.contents.mkdir(parents=True)
        self.plist = self.contents / "Info.plist"
        self.metadata = {
            "CFBundleShortVersionString": "0.1.0",
            "CFBundleVersion": "0.1.0",
            "CFBundleIdentifier": "io.github.develata.rssreader",
            "CFBundleExecutable": "rssr-app",
            "CFBundleName": "RSS-Reader",
            "LSMinimumSystemVersion": "10.13",
            "CustomMetadata": {"enabled": True, "items": ["one", "two"]},
        }
        self.write_plist()
        self.binary = self.contents / "MacOS" / "rssr-app"
        self.binary.parent.mkdir()
        self.binary.write_bytes(b"fixture executable")

    def write_plist(self, fmt=plistlib.FMT_XML):
        self.plist.write_bytes(plistlib.dumps(self.metadata, fmt=fmt))

    def assert_versions(self, metadata, version):
        expected = dict(self.metadata)
        expected.update({key: version for key in VERSION_KEYS})
        self.assertEqual(metadata, expected)

    def test_updates_both_versions_and_preserves_other_metadata(self):
        for fmt in (plistlib.FMT_XML, plistlib.FMT_BINARY):
            with self.subTest(fmt=fmt):
                self.write_plist(fmt)
                self.assertEqual(prepare_bundle(self.bundle, "v0.1.22"), "0.1.22")
                actual = self.plist.read_bytes()
                self.assertEqual(actual.startswith(b"bplist00"), fmt == plistlib.FMT_BINARY)
                self.assert_versions(plistlib.loads(actual), "0.1.22")
                self.assertEqual(self.binary.read_bytes(), b"fixture executable")
                prepare_bundle(self.bundle, "v9.87.65")
                self.assert_versions(plistlib.loads(self.plist.read_bytes()), "9.87.65")

    def test_rejects_invalid_tags_without_mutating(self):
        original = self.plist.read_bytes()
        for tag in ("", "0.1.22", "v0.1", "v0.1.22.0", "v0.1.22-beta.1",
                    "v0.1.22+build", "v00.1.22", "v0.01.22", "v0.1.022",
                    " v0.1.22", "v0.1.22\n", "v0.1.22; echo bad", "v０.1.22"):
            with self.subTest(tag=tag):
                with self.assertRaises(ValueError):
                    prepare_bundle(self.bundle, tag)
                self.assertEqual(self.plist.read_bytes(), original)

    def test_rejects_missing_or_non_string_version_fields(self):
        for key in VERSION_KEYS:
            for value in (None, 100):
                with self.subTest(key=key, value=value):
                    metadata = dict(self.metadata)
                    if value is None:
                        del metadata[key]
                    else:
                        metadata[key] = value
                    original = plistlib.dumps(metadata)
                    self.plist.write_bytes(original)
                    with self.assertRaises(ValueError):
                        prepare_bundle(self.bundle, "v0.1.22")
                    self.assertEqual(self.plist.read_bytes(), original)

    def test_rejects_signed_bundle_without_mutating(self):
        original = self.plist.read_bytes()
        signature = self.contents / "_CodeSignature"
        signature.mkdir()
        seal = signature / "CodeResources"
        seal.write_bytes(b"existing signature")
        with self.assertRaisesRegex(ValueError, "before signing"):
            prepare_bundle(self.bundle, "v0.1.22")
        self.assertEqual(self.plist.read_bytes(), original)
        self.assertEqual(seal.read_bytes(), b"existing signature")

    def test_rejects_legacy_signature_and_symlinked_plist(self):
        signature = self.contents / "CodeResources"
        signature.symlink_to("missing-seal")
        with self.assertRaisesRegex(ValueError, "before signing"):
            prepare_bundle(self.bundle, "v0.1.22")
        signature.unlink()
        target = self.root / "outside.plist"
        self.plist.rename(target)
        self.plist.symlink_to(target)
        original = target.read_bytes()
        with self.assertRaisesRegex(ValueError, "symlinked"):
            prepare_bundle(self.bundle, "v0.1.22")
        self.assertEqual(target.read_bytes(), original)

    def test_rejects_missing_and_malformed_plist(self):
        self.plist.unlink()
        with self.assertRaises(ValueError):
            prepare_bundle(self.bundle, "v0.1.22")
        self.plist.write_bytes(b"not a plist")
        with self.assertRaises(plistlib.InvalidFileException):
            prepare_bundle(self.bundle, "v0.1.22")
        self.assertEqual(self.plist.read_bytes(), b"not a plist")

    def test_cli_and_final_archive_round_trip(self):
        result = subprocess.run(
            [sys.executable, str(Path(__file__).with_name("prepare_macos_bundle.py")),
             str(self.bundle), "--release-tag", "v9.87.65"],
            check=True, capture_output=True, text=True,
        )
        self.assertIn("9.87.65", result.stdout)
        archive_path = self.root / "RSS-Reader-macos-fixture.tar.gz"
        with tarfile.open(archive_path, "w:gz") as archive:
            archive.add(self.bundle, arcname="RSS-Reader.app")
        with tarfile.open(archive_path, "r:gz") as archive:
            with archive.extractfile("RSS-Reader.app/Contents/Info.plist") as source:
                self.assert_versions(plistlib.load(source), "9.87.65")


if __name__ == "__main__":
    unittest.main()
