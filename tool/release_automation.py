"""Publish an unreleased Cargo version from the exact successful CI commit."""
import argparse
import json
import os
from pathlib import Path
import subprocess
import urllib.error
import urllib.request

from build_info import ROOT, version_info
from release_notes import notes, release_assets


class GitHub:
    def __init__(self):
        self.repository = os.environ["GITHUB_REPOSITORY"]

    def get(self, path):
        api = os.environ.get("GITHUB_API_URL", "https://api.github.com")
        request = urllib.request.Request(
            f"{api}/repos/{self.repository}/{path}",
            headers={"Authorization": f"Bearer {os.environ['GH_TOKEN']}",
                     "Accept": "application/vnd.github+json",
                     "X-GitHub-Api-Version": "2022-11-28",
                     "User-Agent": "tst-xiangqi-release"},
        )
        try:
            with urllib.request.urlopen(request, timeout=30) as response:
                return json.load(response)
        except urllib.error.HTTPError as error:
            if error.code == 404:
                return None
            raise RuntimeError(f"GitHub release lookup failed (HTTP {error.code})") from None


def release_plan(api, version, commit, ref):
    tag = "v" + version
    if ref.startswith("refs/tags/") and ref != "refs/tags/" + tag:
        raise ValueError("tag does not match Cargo.toml version")
    release = api.get(f"releases/tags/{tag}")
    # The by-tag endpoint only promises published releases. List releases to
    # recover an interrupted draft even if GitHub has not created its tag yet.
    page = 1
    while release is None:
        releases = api.get(f"releases?per_page=100&page={page}") or []
        release = next((item for item in releases if item["tag_name"] == tag), None)
        if len(releases) < 100:
            break
        page += 1
    if release and not release["draft"]:
        if release["prerelease"]:
            raise ValueError("version already exists as a prerelease")
        return tag, False, False
    existing = api.get(f"git/ref/tags/{tag}")
    if existing:
        target = existing["object"]
        while target["type"] == "tag":
            target = api.get(f"git/tags/{target['sha']}")["object"]
        if target["type"] != "commit" or target["sha"] != commit:
            raise ValueError("unpublished version tag points to a different CI commit")
    if release and release["target_commitish"] != commit and not existing:
        raise ValueError("draft release targets a different CI commit")
    return tag, True, bool(release)


def publish(api, tag, commit, draft_exists, assets, notes_file):
    def gh(*arguments):
        subprocess.run(["gh", "release", *arguments, "--repo", api.repository], check=True)

    # Upload into a draft first: interrupted uploads can be retried without
    # exposing a stable release with missing packages.
    files = sorted(str(p) for p in assets.rglob("*") if p.is_file()
                   and (p.suffix in {".zip", ".apk"} or p.name.endswith(".build.json")
                        or p.name == "SHA256SUMS"))
    if not files:
        raise ValueError("no verified release assets")
    if not draft_exists:
        gh("create", tag, "--target", commit, "--draft", "--title", tag,
           "--notes-file", str(notes_file))
    gh("upload", tag, *files, "--clobber")
    gh("edit", tag, "--draft=false", "--prerelease=false", "--latest",
       "--title", tag, "--notes-file", str(notes_file))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--publish", action="store_true")
    parser.add_argument("--assets", type=Path, default=Path("release-artifacts"))
    parser.add_argument("--notes", type=Path, default=Path("target/release-notes.md"))
    parser.add_argument("--android-build-tools", type=Path)
    args = parser.parse_args()
    version = version_info()["version"]
    body = notes("v" + version, (ROOT / "CHANGELOG.md").read_text(encoding="utf-8-sig"), version)
    commit = os.environ["GITHUB_SHA"]
    api = GitHub()
    tag, should_publish, draft = release_plan(api, version, commit, os.environ["GITHUB_REF"])
    if output := os.environ.get("GITHUB_OUTPUT"):
        with open(output, "a", encoding="utf-8") as stream:
            stream.write(f"tag={tag}\npublish={str(should_publish).lower()}\n")
    print(f"{tag}: {'publish verified CI artifacts' if should_publish else 'already published; skipping'}")
    if args.publish and should_publish:
        if not args.android_build_tools:
            raise ValueError("publishing requires actual Android signature verification")
        release_assets(args.assets, version, args.android_build_tools)
        args.notes.parent.mkdir(parents=True, exist_ok=True)
        args.notes.write_text(body, encoding="utf-8")
        publish(api, tag, commit, draft, args.assets, args.notes)


if __name__ == "__main__":
    main()
