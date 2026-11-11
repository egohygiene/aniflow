"""Authored synthetic reader cases. Not executed in the source-only sprint."""
import io
from pathlib import Path
import sys
import tarfile
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts"))
from release import bundle, library


class LibraryAssetTests(unittest.TestCase):
    def test_source_archive_refuses_untracked_payload_before_publication(self):
        stream = io.BytesIO()
        with tarfile.open(fileobj=stream, mode="w:gz") as archive:
            member = tarfile.TarInfo("aniflow-0.3.0/untracked-secret.txt")
            member.size = 4
            archive.addfile(member, io.BytesIO(b"fake"))
        with patch.object(library, "command", return_value=b"src/lib.rs\0"):
            with self.assertRaisesRegex(ValueError, "exact tracked source"):
                library.observe(stream.getvalue(), "v0.3.0", "a" * 40, ROOT)

    def test_library_reader_does_not_extract_or_follow_link_members(self):
        stream = io.BytesIO()
        with tarfile.open(fileobj=stream, mode="w:gz") as archive:
            member = tarfile.TarInfo("aniflow-0.3.0/src/lib.rs")
            member.type = tarfile.SYMTYPE
            member.linkname = "/outside/private"
            archive.addfile(member)
        with patch.object(library, "command", return_value=b"src/lib.rs\0"):
            with self.assertRaisesRegex(ValueError, "unsafe member"):
                library.observe(stream.getvalue(), "v0.3.0", "a" * 40, ROOT)

    def test_intel_mach_o_cannot_be_substituted_for_apple_silicon(self):
        data = bytearray(32)
        data[:4] = b"\xcf\xfa\xed\xfe"
        data[4:8] = (0x1000007).to_bytes(4, "little")
        bundle.binary_identity(bytes(data), "x86_64-apple-darwin")
        with self.assertRaises(ValueError):
            bundle.binary_identity(bytes(data), "aarch64-apple-darwin")
