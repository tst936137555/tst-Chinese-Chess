import os
from pathlib import Path
import tempfile
import unittest
from unittest.mock import Mock, patch
import urllib.error

from release_automation import GitHub, main, publish, release_plan


class ReleaseAutomationTests(unittest.TestCase):
    def api(self, responses):
        return Mock(repository="owner/repo", get=lambda path: responses.get(path))

    def test_unreleased_main_version_and_explicit_tag_are_publishable(self):
        for ref in ["refs/heads/main", "refs/tags/v2.1.1"]:
            self.assertEqual(release_plan(self.api({}), "2.1.1", "tested", ref),
                             ("v2.1.1", True, False))

    def test_published_version_is_skipped_on_later_commits(self):
        api = self.api({"releases/tags/v2.1.1": {"draft": False, "prerelease": False}})
        self.assertEqual(release_plan(api, "2.1.1", "later", "refs/heads/main"),
                         ("v2.1.1", False, False))

    def test_mismatched_tag_and_prerelease_are_rejected(self):
        with self.assertRaisesRegex(ValueError, "Cargo.toml"):
            release_plan(self.api({}), "2.1.1", "tested", "refs/tags/v2.1.0")
        api = self.api({"releases/tags/v2.1.1": {"draft": False, "prerelease": True}})
        with self.assertRaisesRegex(ValueError, "prerelease"):
            release_plan(api, "2.1.1", "tested", "refs/heads/main")

    def test_existing_lightweight_and_annotated_tags_must_match_tested_commit(self):
        for annotated in [False, True]:
            for sha in ["tested", "other"]:
                responses = {"git/ref/tags/v2.1.1": {"object": {
                    "type": "tag" if annotated else "commit", "sha": "annotation" if annotated else sha}},
                    "git/tags/annotation": {"object": {"type": "commit", "sha": sha}}}
                with self.subTest(annotated=annotated, sha=sha):
                    if sha == "other":
                        with self.assertRaisesRegex(ValueError, "different CI commit"):
                            release_plan(self.api(responses), "2.1.1", "tested", "refs/heads/main")
                    else:
                        self.assertTrue(release_plan(self.api(responses), "2.1.1", "tested", "refs/heads/main")[1])

    def test_draft_can_resume_only_for_its_tested_commit(self):
        for target in ["tested", "other"]:
            api = self.api({"releases/tags/v2.1.1": {"draft": True, "target_commitish": target}})
            if target == "tested":
                self.assertEqual(release_plan(api, "2.1.1", "tested", "refs/heads/main"),
                                 ("v2.1.1", True, True))
            else:
                with self.assertRaisesRegex(ValueError, "different CI commit"):
                    release_plan(api, "2.1.1", "tested", "refs/heads/main")

    def test_draft_missing_from_by_tag_endpoint_is_recovered_from_list(self):
        draft = {"tag_name": "v2.1.1", "draft": True, "target_commitish": "tested"}
        api = self.api({"releases?per_page=100&page=1": [{"tag_name": "other"}] * 100,
                        "releases?per_page=100&page=2": [draft]})
        self.assertEqual(release_plan(api, "2.1.1", "tested", "refs/heads/main"),
                         ("v2.1.1", True, True))

    def test_lookup_failure_is_not_treated_as_unreleased(self):
        with patch.dict(os.environ, {"GITHUB_REPOSITORY": "owner/repo", "GH_TOKEN": "fixture"}):
            for status in [403, 404, 500]:
                error = urllib.error.HTTPError("https://api.github.com", status, "fixture", {}, None)
                with patch("urllib.request.urlopen", side_effect=error):
                    if status == 404:
                        self.assertIsNone(GitHub().get("releases/tags/v2.1.1"))
                    else:
                        with self.assertRaisesRegex(RuntimeError, f"HTTP {status}"):
                            GitHub().get("releases/tags/v2.1.1")

    def test_publication_waits_for_all_uploads_and_resumes_drafts(self):
        with tempfile.TemporaryDirectory() as tmp:
            assets = Path(tmp)
            (assets / "package.zip").write_bytes(b"fixture")
            for resume in [False, True]:
                with patch("release_automation.subprocess.run") as run:
                    publish(self.api({}), "v2.1.1", "tested", resume, assets, Path("notes.md"))
                    commands = [call.args[0] for call in run.call_args_list]
                    self.assertEqual([c[2] for c in commands], ["upload", "edit"] if resume else ["create", "upload", "edit"])
                    if not resume:
                        self.assertIn("--draft", commands[0])
                        self.assertEqual(commands[0][commands[0].index("--target") + 1], "tested")
                with patch("release_automation.subprocess.run", side_effect=RuntimeError("upload failed")) as run:
                    with self.assertRaisesRegex(RuntimeError, "upload failed"):
                        publish(self.api({}), "v2.1.1", "tested", True, assets, Path("notes.md"))
                    self.assertEqual(run.call_count, 1)

    def test_invalid_assets_cannot_create_tag_or_release(self):
        environment = {"GITHUB_SHA": "tested", "GITHUB_REF": "refs/heads/main"}
        with patch.dict(os.environ, environment), patch("sys.argv", ["release_automation.py", "--publish", "--android-build-tools", "sdk"]), \
                patch("release_automation.GitHub", return_value=self.api({})), \
                patch("release_automation.release_assets", side_effect=ValueError("invalid signature")), \
                patch("release_automation.publish") as do_publish:
            with self.assertRaisesRegex(ValueError, "invalid signature"):
                main()
            do_publish.assert_not_called()


if __name__ == "__main__":
    unittest.main()
