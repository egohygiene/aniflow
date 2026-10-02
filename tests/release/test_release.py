"""Synthetic contract tests. Authored for #10; execution deferred under #64."""

import io
import json
from pathlib import Path
import re
import shutil
import sys
import tarfile
import tempfile
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts"))
from release import bundle, plan, remote
from release.common import checksum_bytes, inventory, json_bytes, new_directory, verify_inventory

REVISION = "a" * 40
TARGET = "x86_64-unknown-linux-gnu"


def synthetic_elf():
    data = bytearray(64)
    data[:6] = b"\x7fELF\x02\x01"
    data[18:20] = (62).to_bytes(2, "little")
    return bytes(data)


class ReleasePlanTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(dir=Path(tempfile.gettempdir()).resolve())
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name) / "repository"
        self.root.mkdir()
        for name in ["Cargo.toml", "Cargo.lock", "CHANGELOG.md", "Taskfile.yml", ".egohygiene", ".github/workflows", "third_party/release"]:
            source, destination = ROOT / name, self.root / name
            destination.parent.mkdir(parents=True, exist_ok=True)
            if source.is_dir():
                shutil.copytree(source, destination)
            else:
                shutil.copyfile(source, destination)
        # The fixture remains a pre-preparation baseline after future releases.
        (self.root / plan.CANDIDATE).unlink(missing_ok=True)
        self.rewrite("Cargo.toml", lambda text: re.sub(r'(?m)^version = "[^"]+"', 'version = "0.3.0"', text, count=1))
        self.rewrite("Cargo.lock", lambda text: re.sub(r'(name = "aniflow"\nversion = ")[^"]+', lambda match: match.group(1) + "0.3.0", text, count=1))
        (self.root / "CHANGELOG.md").write_text("# Changelog\n\n## [Unreleased]\n\n### Added\n\n- Synthetic change.\n\n## [0.3.0] - 2026-08-14\n\n### Added\n\n- Historical synthetic record.\n", encoding="utf-8")

    def rewrite(self, name, transform):
        path = self.root / name
        path.write_text(transform(path.read_text()), encoding="utf-8")

    def test_authority_is_one_cargo_component_and_no_publication_claim(self):
        result = plan.check(self.root)
        self.assertEqual(result["publication"], "not-performed")
        self.assertEqual(result["qualification"], "not-established")
        self.assertEqual(result["declaration"]["components"][0]["version_authority"]["selector"], "package.version")

    def test_lockfile_drift_is_rejected(self):
        self.rewrite("Cargo.lock", lambda text: text.replace('name = "aniflow"\nversion = "', 'name = "aniflow"\nversion = "99.'))
        with self.assertRaisesRegex(ValueError, "Cargo.lock version drift"):
            plan.check(self.root)

    def test_declared_authority_cannot_be_switched_to_a_convenient_file(self):
        path = self.root / plan.DECLARATION
        declaration = json.loads(path.read_text())
        declaration["components"][0]["version_authority"]["path"] = "Cargo.lock"
        path.write_bytes(json_bytes(declaration))
        with self.assertRaisesRegex(ValueError, "sole aniflow authority"):
            plan.check(self.root)

    def test_unknown_declaration_field_fails_full_schema(self):
        from jsonschema.exceptions import ValidationError
        path = self.root / plan.DECLARATION
        declaration = json.loads(path.read_text())
        declaration["publish_now"] = True
        path.write_bytes(json_bytes(declaration))
        with self.assertRaises(ValidationError):
            plan.check(self.root)

    def test_policy_snapshot_drift_is_rejected(self):
        self.rewrite("third_party/release/aether/schema.json", lambda text: text + " ")
        with self.assertRaisesRegex(ValueError, "policy snapshot drift"):
            plan.check(self.root)

    def test_latest_changelog_must_match_cargo(self):
        self.rewrite("CHANGELOG.md", lambda text: text.replace("## [0.3.0]", "## [99.0.0]", 1))
        with self.assertRaisesRegex(ValueError, "changelog version drift"):
            plan.check(self.root)

    def test_duplicate_unreleased_is_rejected(self):
        self.rewrite("CHANGELOG.md", lambda text: text + "\n## [Unreleased]\n")
        with self.assertRaisesRegex(ValueError, "exactly one Unreleased"):
            plan.check(self.root)

    def test_prepare_promotes_reviewed_text_and_retains_history(self):
        before = (self.root / "CHANGELOG.md").read_text()
        historical = before[before.index("## [0.3.0]"):]
        with patch.object(plan, "clean_revision", return_value=REVISION):
            result = plan.prepare("v99.0.0", "2026-10-02", self.root)
        self.assertEqual(len(result["changed_paths"]), 4)
        after = (self.root / "CHANGELOG.md").read_text()
        self.assertTrue(after.endswith(historical))
        self.assertIn("## [Unreleased]\n\n## [99.0.0] - 2026-10-02", after)
        self.assertEqual(plan.check(self.root)["version"], "v99.0.0")

    def test_invalid_version_date_and_dirty_prepare_do_not_mutate(self):
        before = inventory(self.root)
        with patch.object(plan, "clean_revision", return_value=REVISION):
            for version, date in [("v0.3.0", "2026-10-02"), ("v99.0.0-rc.1", "2026-10-02"), ("v99.0.0", "2026-02-30")]:
                with self.assertRaises(ValueError):
                    plan.prepare(version, date, self.root)
                self.assertEqual(inventory(self.root), before)
        with patch.object(plan, "clean_revision", side_effect=ValueError("dirty tree")):
            with self.assertRaisesRegex(ValueError, "dirty tree"):
                plan.prepare("v99.0.0", "2026-10-02", self.root)
        self.assertEqual(inventory(self.root), before)

    def test_historical_version_is_not_a_prepared_release(self):
        with self.assertRaisesRegex(ValueError, "prepare and review"):
            plan.verify_candidate(plan.check(self.root)["version"], REVISION, self.root)

    def test_requested_tag_cannot_disagree_with_prepared_candidate(self):
        with patch.object(plan, "clean_revision", return_value=REVISION):
            plan.prepare("v99.0.0", "2026-10-02", self.root)
            with self.assertRaisesRegex(ValueError, "requested tag"):
                plan.verify_candidate("v98.0.0", REVISION, self.root)

    def test_new_unreleased_changes_block_prepared_candidate(self):
        with patch.object(plan, "clean_revision", return_value=REVISION):
            plan.prepare("v99.0.0", "2026-10-02", self.root)
            self.rewrite("CHANGELOG.md", lambda text: text.replace("## [Unreleased]\n", "## [Unreleased]\n\n- New change\n", 1))
            with self.assertRaisesRegex(ValueError, "unpromoted changes"):
                plan.verify_candidate("v99.0.0", REVISION, self.root)


class BundleTests(unittest.TestCase):
    def test_archive_is_repeatable_across_insertion_order(self):
        files = {"LICENSE": (b"synthetic", 0o644), "bin/aniflow": (synthetic_elf(), 0o755)}
        first = bundle.archive_bytes(files)
        second = bundle.archive_bytes(dict(reversed(list(files.items()))))
        self.assertEqual(first, second)
        self.assertEqual(bundle.archive_inventory(first, TARGET)["bin/aniflow"], synthetic_elf())

    def test_archive_refuses_traversal_and_incorrect_architecture(self):
        data = bundle.archive_bytes({"../escape": (b"no", 0o644), "bin/aniflow": (synthetic_elf(), 0o755)})
        with self.assertRaisesRegex(ValueError, "escapes"):
            bundle.archive_inventory(data, TARGET)
        data = bundle.archive_bytes({"bin/aniflow": (b"not executable", 0o755)})
        with self.assertRaisesRegex(ValueError, "ELF"):
            bundle.archive_inventory(data, TARGET)

    def test_archive_refuses_symlink_members(self):
        stream = io.BytesIO()
        with tarfile.open(fileobj=stream, mode="w:gz") as archive:
            member = tarfile.TarInfo("bin/aniflow")
            member.type, member.linkname = tarfile.SYMTYPE, "/outside"
            archive.addfile(member)
        with self.assertRaisesRegex(ValueError, "non-regular"):
            bundle.archive_inventory(stream.getvalue(), TARGET)

    def test_complete_checksums_refuse_tamper_and_unlisted_files(self):
        with tempfile.TemporaryDirectory(dir=Path(tempfile.gettempdir()).resolve()) as directory:
            root = Path(directory)
            (root / "payload").write_bytes(b"synthetic")
            (root / "SHA256SUMS").write_bytes(checksum_bytes(inventory(root)))
            verify_inventory(root)
            (root / "extra").write_bytes(b"not declared")
            with self.assertRaisesRegex(ValueError, "every file"):
                verify_inventory(root)
            (root / "extra").unlink()
            (root / "payload").write_bytes(b"altered")
            with self.assertRaisesRegex(ValueError, "matching bytes"):
                verify_inventory(root)

    def test_existing_output_is_never_adopted(self):
        with tempfile.TemporaryDirectory(dir=Path(tempfile.gettempdir()).resolve()) as directory:
            root = Path(directory)
            (root / "sentinel").write_bytes(b"preserve")
            with self.assertRaises(FileExistsError):
                new_directory(root)
            self.assertEqual((root / "sentinel").read_bytes(), b"preserve")

    def test_inventory_does_not_follow_links(self):
        with tempfile.TemporaryDirectory(dir=Path(tempfile.gettempdir()).resolve()) as directory:
            root = Path(directory)
            (root / "link").symlink_to(ROOT / "Cargo.toml")
            with self.assertRaisesRegex(ValueError, "symlink"):
                inventory(root)

    def test_partial_target_matrix_is_not_a_release(self):
        with tempfile.TemporaryDirectory(dir=Path(tempfile.gettempdir()).resolve()) as directory:
            root = Path(directory)
            (root / ("native." + TARGET)).mkdir()
            with patch.object(bundle, "verify_candidate", return_value={"build": {"targets": [{"triple": TARGET}, {"triple": "aarch64-apple-darwin"}]}}):
                with self.assertRaisesRegex(ValueError, "target set"):
                    bundle.assemble(root, root / "output", "v0.4.0", REVISION)
            self.assertFalse((root / "output").exists())

    def test_signed_payload_digest_is_recomputed_before_trusting_provenance(self):
        with tempfile.TemporaryDirectory(dir=Path(tempfile.gettempdir()).resolve()) as directory:
            root = Path(directory)
            (root / "payload-SHA256SUMS").write_bytes(b"invented\n")
            (root / "provenance.json").write_bytes(json_bytes({"source_revision": REVISION}))
            with self.assertRaisesRegex(ValueError, "signed payload inventory mismatch"):
                bundle.verify_payload(root, "v0.4.0", REVISION)

    def test_signature_verification_binds_workflow_source_and_main(self):
        with patch.object(bundle, "command", return_value=b"[]") as invoke:
            bundle.verify_signature(Path("/synthetic/bundle"), REVISION)
        arguments = invoke.call_args.args[0]
        for flag, value in [("--source-digest", REVISION), ("--signer-digest", REVISION),
                            ("--source-ref", "refs/heads/main"),
                            ("--signer-workflow", "egohygiene/aniflow/.github/workflows/release-candidate.yml")]:
            self.assertEqual(arguments[arguments.index(flag) + 1], value)
        with patch.object(bundle, "command", side_effect=ValueError("signature rejected")):
            with self.assertRaisesRegex(ValueError, "signature rejected"):
                bundle.verify_signature(Path("/synthetic/bundle"), REVISION)


class RemoteGateTests(unittest.TestCase):
    def setUp(self):
        self.environment = {"GITHUB_EVENT_NAME": "workflow_dispatch", "GITHUB_REPOSITORY": "egohygiene/aniflow",
                            "GITHUB_REF": "refs/heads/main", "GITHUB_SHA": REVISION}
        self.plan = {"build": {"first_release_gates": [{"repository": "egohygiene/egolint", "issue": 29}],
                               "candidate_workflow": ".github/workflows/release-candidate.yml"}}

    def test_open_dependency_blocks_release(self):
        replies = [{"default_branch": "main"}, {"commit": {"sha": REVISION}}, {"state": "open"}]
        with patch.dict("os.environ", self.environment), patch.object(remote, "verify_candidate", return_value=self.plan), patch.object(remote, "api", side_effect=replies):
            with self.assertRaisesRegex(ValueError, "dependency remains incomplete"):
                remote.guard("v0.4.0", REVISION)

    def test_main_advancing_invalidates_candidate(self):
        replies = [{"default_branch": "main"}, {"commit": {"sha": "b" * 40}}]
        with patch.dict("os.environ", self.environment), patch.object(remote, "verify_candidate", return_value=self.plan), patch.object(remote, "api", side_effect=replies):
            with self.assertRaisesRegex(ValueError, "current default-branch"):
                remote.guard("v0.4.0", REVISION)

    def test_different_source_tag_is_never_overwritten(self):
        replies = [{"default_branch": "main"}, {"commit": {"sha": REVISION}}, {"state": "closed", "state_reason": "completed"},
                   [{"ref": "refs/tags/v0.4.0", "object": {"type": "commit", "sha": "b" * 40}}]]
        with patch.dict("os.environ", self.environment), patch.object(remote, "verify_candidate", return_value=self.plan), patch.object(remote, "api", side_effect=replies):
            with self.assertRaisesRegex(ValueError, "immutable tag"):
                remote.guard("v0.4.0", REVISION)

    def test_successful_foreign_workflow_cannot_supply_candidate(self):
        replies = [{"default_branch": "main"}, {"commit": {"sha": REVISION}}, {"state": "closed", "state_reason": "completed"}, [],
                   {"head_sha": REVISION, "head_branch": "main", "event": "workflow_dispatch", "conclusion": "success",
                    "path": ".github/workflows/unrelated.yml"}]
        with patch.dict("os.environ", self.environment), patch.object(remote, "verify_candidate", return_value=self.plan), patch.object(remote, "api", side_effect=replies):
            with self.assertRaisesRegex(ValueError, "successful manual qualification"):
                remote.guard("v0.4.0", REVISION, "123")


if __name__ == "__main__":
    unittest.main()
