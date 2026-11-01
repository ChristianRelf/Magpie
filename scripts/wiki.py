#!/usr/bin/env python3
"""Check, render, and publish the versioned GitHub wiki using only stdlib + Git."""

from __future__ import annotations

import argparse
import json
import re
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path
from urllib.parse import quote, unquote, urlsplit

ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "docs" / "wiki"
DEFAULT_REPO = "ChristianRelf/Magpie"
LINK = re.compile(r"\[([^\]\n]+)\]\(([^\s)]+)\)")


class WikiError(Exception):
    pass


def command(args: list[str], *, cwd: Path = ROOT, allowed: tuple[int, ...] = (0,)) -> subprocess.CompletedProcess[str]:
    result = subprocess.run(args, cwd=cwd, text=True, capture_output=True, check=False)
    if result.returncode not in allowed:
        raise WikiError(f"{' '.join(args[:3])} failed: {result.stderr.strip() or result.stdout.strip()}")
    return result


def sections(content: str) -> tuple[str, list[tuple[str, str]]]:
    """Extract fenced examples and leave prose for link/heading checks."""
    prose: list[str] = []
    blocks: list[tuple[str, str]] = []
    marker = ""
    language = ""
    body: list[str] = []
    for line in content.splitlines(keepends=True):
        fence = re.match(r"^\s{0,3}(`{3,}|~{3,})(.*)$", line.rstrip("\n"))
        if not marker and fence:
            marker, language = fence.group(1), fence.group(2).strip()
            body = []
        elif marker and fence and fence.group(1)[0] == marker[0] and len(fence.group(1)) >= len(marker) and not fence.group(2).strip():
            blocks.append((language, "".join(body)))
            marker = ""
        elif marker:
            body.append(line)
        else:
            prose.append(line)
    if marker:
        raise WikiError("Unclosed fenced code block")
    return "".join(prose), blocks


def anchors(content: str) -> set[str]:
    prose, _ = sections(content)
    found: set[str] = set()
    for heading in re.findall(r"^#{1,6}\s+(.+?)\s*#*\s*$", prose, flags=re.MULTILINE):
        heading = LINK.sub(lambda match: match.group(1), heading)
        heading = re.sub(r"<[^>]*>", "", heading).lower()
        base = re.sub(r"[^\w\- ]", "", heading).replace(" ", "-")
        slug, suffix = base, 0
        while slug in found:
            suffix += 1
            slug = f"{base}-{suffix}"
        found.add(slug)
    return found


def pages() -> list[Path]:
    result = sorted(SOURCE.glob("*.md"))
    if not result:
        raise WikiError(f"No wiki Markdown found in {SOURCE}")
    for page in result:
        if page.is_symlink() or not re.fullmatch(r"[A-Za-z0-9_-]+\.md", page.name):
            raise WikiError(f"Unsupported wiki filename or symlink: {page.name}")
    return result


def check() -> list[Path]:
    wiki_pages = pages()
    files = [ROOT / "README.md", *wiki_pages]
    errors: list[str] = []
    counts = {"links": 0, "json": 0, "bash": 0}
    bash = shutil.which("bash")
    for path in files:
        label = str(path.relative_to(ROOT))
        content = path.read_text(encoding="utf-8")
        try:
            prose, blocks = sections(content)
        except WikiError as error:
            errors.append(f"{label}: {error}")
            continue
        if not content.endswith("\n"):
            errors.append(f"{label}: missing final newline")
        for match in LINK.finditer(prose):
            href = match.group(2).strip("<>")
            parsed = urlsplit(href)
            if parsed.scheme or parsed.netloc:
                continue
            counts["links"] += 1
            target = (path.parent / unquote(parsed.path)).resolve() if parsed.path else path
            if not target.is_relative_to(ROOT) or not target.exists():
                errors.append(f"{label}: missing/invalid local target {href}")
                continue
            if parsed.fragment and target.suffix == ".md":
                if unquote(parsed.fragment) not in anchors(target.read_text(encoding="utf-8")):
                    errors.append(f"{label}: missing heading in {href}")
        for language, body in blocks:
            if language == "json":
                counts["json"] += 1
                try:
                    json.loads(body)
                except json.JSONDecodeError as error:
                    errors.append(f"{label}: invalid JSON example: {error}")
            elif language == "bash" and bash:
                counts["bash"] += 1
                result = subprocess.run([bash, "-n"], input=body, text=True, capture_output=True, check=False)
                if result.returncode:
                    errors.append(f"{label}: invalid Bash syntax: {result.stderr.strip()}")
    names = {page.name for page in wiki_pages}
    for required in ("Home.md", "_Sidebar.md", "_Footer.md"):
        if required not in names:
            errors.append(f"Missing {required}")
    for navigation in ("Home.md", "_Sidebar.md"):
        if navigation not in names:
            continue
        prose, _ = sections((SOURCE / navigation).read_text(encoding="utf-8"))
        targets = {urlsplit(match.group(2)).path for match in LINK.finditer(prose)}
        for page in wiki_pages:
            if page.name != "Home.md" and not page.name.startswith("_") and page.name not in targets:
                errors.append(f"{navigation}: missing navigation link to {page.name}")
    if errors:
        raise WikiError("\n".join(errors))
    count = sum(not page.name.startswith("_") for page in wiki_pages)
    print(f"Checked README and {count} wiki pages + sidebar/footer: {counts['links']} local links, {counts['json']} JSON examples, {counts['bash']} Bash blocks.")
    if not bash:
        print("Bash unavailable: shell example syntax checks skipped.")
    return wiki_pages


def render(page: Path, repo: str) -> str:
    """Keep repository .md links readable locally and turn them into wiki URLs."""
    def replace(match: re.Match[str]) -> str:
        href = match.group(2)
        parsed = urlsplit(href)
        if parsed.scheme or parsed.netloc or not parsed.path:
            return match.group(0)
        target = (page.parent / unquote(parsed.path)).resolve()
        if target.parent == SOURCE and target.suffix == ".md":
            url = f"https://github.com/{repo}/wiki/{quote(target.stem)}"
        else:
            url = f"https://github.com/{repo}/blob/main/{quote(target.relative_to(ROOT).as_posix(), safe='/')}"
        if parsed.fragment:
            url += "#" + parsed.fragment
        return f"[{match.group(1)}]({url})"

    output: list[str] = []
    marker = ""
    for line in page.read_text(encoding="utf-8").splitlines(keepends=True):
        fence = re.match(r"^\s{0,3}(`{3,}|~{3,})(.*)$", line.rstrip("\n"))
        if not marker and fence:
            marker = fence.group(1)
            output.append(line)
        elif marker:
            output.append(line)
            if fence and fence.group(1)[0] == marker[0] and len(fence.group(1)) >= len(marker) and not fence.group(2).strip():
                marker = ""
        else:
            output.append(LINK.sub(replace, line))
    return "".join(output)


def write_pages(destination: Path, wiki_pages: list[Path], repo: str) -> None:
    for page in wiki_pages:
        target = destination / page.name
        if target.is_symlink():
            raise WikiError(f"Refusing to overwrite a symlink: {target}")
        target.write_text(render(page, repo), encoding="utf-8")


def publish(wiki_pages: list[Path], repo: str, push: bool) -> None:
    remote = f"https://github.com/{repo}.wiki.git"
    with tempfile.TemporaryDirectory(prefix="magpie-wiki-") as temporary:
        checkout = Path(temporary) / "wiki"
        try:
            command(["git", "clone", "--quiet", "--depth", "1", remote, str(checkout)])
        except WikiError as error:
            raise WikiError(
                f"Cannot clone {remote}. Enable the wiki and create its first Home page in a signed-in browser, "
                "then verify Git authentication and retry. No pages were published.\n" + str(error)
            ) from error
        branch = command(["git", "branch", "--show-current"], cwd=checkout).stdout.strip()
        if not branch:
            raise WikiError("Wiki clone has no default branch; refusing to guess a publishing branch")
        write_pages(checkout, wiki_pages, repo)
        command(["git", "add", "--", *[page.name for page in wiki_pages]], cwd=checkout)
        changed = command(["git", "diff", "--cached", "--quiet"], cwd=checkout, allowed=(0, 1)).returncode
        if not changed:
            print("Published wiki already matches the checked source. No changes needed.")
            return
        print(command(["git", "diff", "--cached", "--stat"], cwd=checkout).stdout.strip())
        if not push:
            print(f"Dry run only: would update {repo} wiki branch {branch}. Pass --push to publish.")
            return
        identity: list[str] = []
        for key in ("user.name", "user.email"):
            value = command(["git", "config", "--get", key], allowed=(0, 1)).stdout.strip()
            if not value:
                raise WikiError(f"Set Git {key} in this repository before publishing; no pages were pushed")
            identity.extend(["-c", f"{key}={value}"])
        command(["git", *identity, "commit", "-m", "docs: publish comprehensive Magpie handbook"], cwd=checkout)
        command(["git", "push", "origin", f"HEAD:refs/heads/{branch}"], cwd=checkout)
        local = command(["git", "rev-parse", "HEAD"], cwd=checkout).stdout.strip()
        remote_head = command(["git", "ls-remote", "origin", f"refs/heads/{branch}"], cwd=checkout).stdout.split()
        if not remote_head or remote_head[0] != local:
            raise WikiError("Push completed but remote head changed before verification; inspect the wiki history")
        print(f"Published and verified {local}\nhttps://github.com/{repo}/wiki")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo", default=DEFAULT_REPO, help="GitHub OWNER/REPOSITORY (default: %(default)s)")
    commands = parser.add_subparsers(dest="action", required=True)
    commands.add_parser("check", help="Check local links, headings, navigation, JSON, and Bash syntax")
    stage = commands.add_parser("stage", help="Render wiki pages into a new/empty directory outside the repository")
    stage.add_argument("--output", required=True, type=Path)
    publish_parser = commands.add_parser("publish", help="Clone and show a wiki diff; publish only with --push")
    publish_parser.add_argument("--push", action="store_true", help="Commit and push a non-forced update to the wiki default branch")
    args = parser.parse_args()
    if not re.fullmatch(r"[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+", args.repo):
        parser.error("--repo must be OWNER/REPOSITORY")
    wiki_pages = check()
    if args.action == "stage":
        destination = args.output.expanduser().resolve()
        if destination.is_relative_to(ROOT):
            raise WikiError("Choose a staging directory outside the source repository")
        if destination.exists() and (not destination.is_dir() or any(destination.iterdir())):
            raise WikiError("Staging output must be a new or empty directory; existing files were not changed")
        destination.mkdir(parents=True, exist_ok=True)
        write_pages(destination, wiki_pages, args.repo)
        print(f"Rendered {len(wiki_pages)} Markdown files to {destination}")
    elif args.action == "publish":
        publish(wiki_pages, args.repo, args.push)
    return 0


if __name__ == "__main__":
    try:
        sys.exit(main())
    except (WikiError, OSError) as error:
        print(f"wiki: {error}", file=sys.stderr)
        sys.exit(1)
