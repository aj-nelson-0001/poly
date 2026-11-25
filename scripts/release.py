#!/usr/bin/env python3
"""Cut a Poly release end to end: prepare, commit, push, wait for CI, tag.

``scripts/prepare_release.py`` stops at "ready to commit". This driver picks up
there and closes the gap that cost a tag rewrite for 2.0.0-preview.20: the tag
was pushed before the branch CI for the release commit had finished, and the
wasm drift gate then failed on the tag itself, so the tag had to be deleted and
re-cut. Here the tag is created only after the CI run for the exact release
commit is green.

    1. Check the preconditions: ``gh`` is signed in, the branch is not
       ``master``, the version is sane, and the tag is free — or, with
       ``--resume``, already names HEAD, which means a run was interrupted
       after tagging and only the release wait is left.
    2. Prepare with ``prepare_release.py <version>`` (skipped by ``--resume``
       when the release commit is already pushed).
    3. Commit ``chore(release): prepare <version>`` and push the branch. Only
       release metadata may be in that commit; anything else aborts first.
    4. Check the wasm size quoted in ``POLY_V2_PREVIEW_RELEASE_NOTES.md``
       against the module ``prepare_release.py`` just built. A stale number is
       a docs bug, but it ships as a false claim, so it stops the run.
    5. Wait for the ``CI`` run on that commit. A red run stops here, before any
       tag exists.
    6. Tag ``<version>`` (annotated) and push the tag, which starts
       ``release.yml``. Skipped when the tag already names HEAD.
    7. Wait for the release run, print the published assets, and report how
       long the prepare, CI, and release stages took.

It never force-pushes and never deletes a tag: a red run leaves the commit (or
the tag) in place with instructions, because moving a tag is a human call.

Usage:

    python3 scripts/release.py 2.0.0-preview.21
    python3 scripts/release.py 2.0.0-preview.21 --skip-tests
    python3 scripts/release.py 2.0.0-preview.21 --resume
    python3 scripts/release.py 2.0.0-preview.21 --resume   # after the tag too
    python3 scripts/release.py 2.0.0-preview.21 --dry-run
"""

from __future__ import annotations

import argparse
import json
import re
import subprocess
import sys
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
PREPARE_SCRIPT = ROOT / "scripts" / "prepare_release.py"
CI_WORKFLOW = "ci.yml"
RELEASE_WORKFLOW = "release.yml"
# The gate's own budget, read rather than repeated, so the summary cannot drift
# from the number that actually fails a build.
WASM_WORKFLOW = ROOT / ".github" / "workflows" / "wasm-build.yml"
WASM_BUDGET_RE = re.compile(r"WASM_SIZE_BUDGET:\s*'(\d+)'")
WASM_MODULE = ROOT / "playground" / "poly.wasm"
RELEASE_NOTES = ROOT / "POLY_V2_PREVIEW_RELEASE_NOTES.md"
# The evidence section quotes the published module, e.g.
# "`poly.wasm: 548185 bytes (budget: 660000)`". The number is written by hand,
# so it has already gone stale once (548,899 for a 548,185-byte module).
NOTES_WASM_RE = re.compile(r"poly\.wasm:\s*(\d[\d,]*)\s*bytes")
POLL_SECONDS = 10
# Measured on 2.0.0-preview.21 (2026-11-25): CI 6m 09s, release 2m 06s. 20
# minutes is about 3x the slower stage, which leaves room for a cold runner
# queue while still failing long before the old 45-minute guess could hang a
# terminal. Override with --timeout when the runners are slow.
DEFAULT_TIMEOUT_MINUTES = 20
VERSION_RE = re.compile(r"^\d+\.\d+\.\d+(?:-[0-9A-Za-z.]+)?$")
# The release commit is metadata only. Docs are edited by hand before the
# driver runs, so they are allowed; application sources are not, and an
# unrelated fix swept into a release commit is exactly the kind of mistake a
# driver should refuse to make.
RELEASE_SURFACE = (
    "CHANGELOG.md",
    "README.md",
    "POLY_DOCUMENTATION_INDEX.md",
    "POLY_V2_PREVIEW_RELEASE_NOTES.md",
    "POLY_RELEASE_GUIDE.md",
    "compiler/Cargo.toml",
    "compiler/Cargo.lock",
    "playground/index.html",
    "playground/poly.wasm",
    "tetris/README.md",
    "tetris/rust_output/tetris/Cargo.toml",
    "tetris/rust_output/tetris/Cargo.lock",
    "tetris/rust_output/tetris/src/main.rs",
    "tetris/rust_output/tetris/.poly-generated",
)


def run(cmd: list[str], allow_failure: bool = False) -> str:
    print(f"  $ {' '.join(cmd)}")
    result = subprocess.run(cmd, cwd=ROOT, capture_output=True, text=True)
    if result.returncode and not allow_failure:
        tail = (result.stderr or result.stdout).strip().splitlines()
        detail = "\n".join(f"    {line}" for line in tail[-10:])
        raise SystemExit(f"command failed ({result.returncode}):\n{detail}")
    return result.stdout


def git(*args: str, allow_failure: bool = False) -> str:
    return run(["git", *args], allow_failure=allow_failure)


def gh_json(*args: str) -> object:
    return json.loads(run(["gh", *args]))


def release_surface_problems(changed: list[str]) -> list[str]:
    """Changed paths the driver refuses to put in a release commit."""
    return [path for path in changed if path not in RELEASE_SURFACE]


def changed_paths() -> list[str]:
    output = git("status", "--porcelain=v1", "--untracked-files=all")
    paths: list[str] = []
    for line in output.splitlines():
        if not line.strip():
            continue
        # Format: XY <path> (renames add " -> <new path>").
        path = line[3:]
        if " -> " in path:
            path = path.split(" -> ", 1)[1]
        paths.append(path)
    return sorted(paths)


def tag_exists(version: str) -> bool:
    local = git("rev-parse", "-q", "--verify", f"refs/tags/{version}", allow_failure=True)
    if local.strip():
        return True
    remote = git("ls-remote", "--tags", "origin", f"refs/tags/{version}", allow_failure=True)
    return bool(remote.strip())


def tag_commit(version: str) -> str | None:
    """The commit a tag names (peeled), or None when it cannot be resolved.

    The tag may exist only on the remote, so both the local ref and
    ``ls-remote``'s peeled entry are consulted. Annotated tags peel to their
    commit; a lightweight tag is its own commit.
    """
    local = git("rev-parse", "-q", "--verify", f"refs/tags/{version}^{{commit}}",
                allow_failure=True).strip()
    if local:
        return local
    for pattern in (f"refs/tags/{version}^{{}}", f"refs/tags/{version}"):
        remote = git("ls-remote", "origin", pattern, allow_failure=True).strip()
        if remote:
            return remote.split()[0]
    return None


def find_run(workflow: str, *, commit: str | None, branch: str) -> dict[str, object]:
    """The push run of `workflow` for `branch` (and `commit`, when given)."""
    print(f"  looking for the {workflow} run on {branch}{f' at {commit[:9]}' if commit else ''}")
    deadline = time.time() + 180
    while time.time() < deadline:
        cmd = ["run", "list", "--workflow", workflow, "--limit", "30", "--json",
               "databaseId,status,conclusion,headBranch,headSha,event,url"]
        if commit:
            cmd += ["--commit", commit]
        for candidate in gh_json(*cmd):  # type: ignore[union-attr]
            if candidate["event"] != "push":
                continue
            if candidate["headBranch"] != branch:
                continue
            if commit and candidate["headSha"] != commit:
                continue
            return candidate  # type: ignore[return-value]
        time.sleep(POLL_SECONDS)
    raise SystemExit(
        f"no {workflow} push run appeared for {branch} within 3 minutes; "
        "check `gh run list` and the workflow's trigger"
    )


def wait_run(run_info: dict[str, object], label: str, timeout_minutes: int) -> dict[str, object]:
    run_id = run_info["databaseId"]
    print(f"  waiting on {label} run {run_id}: {run_info.get('url', '')}")
    deadline = time.time() + timeout_minutes * 60
    last_status = None
    while time.time() < deadline:
        view = gh_json("run", "view", str(run_id), "--json", "status,conclusion,jobs")
        status = view["status"]  # type: ignore[index]
        if status != last_status:
            print(f"  {label} run {run_id}: {status}")
            last_status = status
        if status == "completed":
            return view  # type: ignore[return-value]
        time.sleep(POLL_SECONDS)
    raise SystemExit(
        f"{label} run {run_id} did not finish within {timeout_minutes} minutes; "
        f"watch it with `gh run watch {run_id}`"
    )


def duration(seconds: float) -> str:
    """A compact "42.3s" / "3m 07.1s" for the stage timings."""
    if seconds < 60:
        return f"{seconds:.1f}s"
    minutes, rest = divmod(seconds, 60)
    return f"{int(minutes)}m {rest:04.1f}s"


def check_notes_wasm_size() -> None:
    """Stop when the release notes quote a wasm size the built module lacks.

    Only the ``poly.wasm: <n> bytes`` evidence line is checked; the notes are
    otherwise prose and are not second-guessed here. A notes file with no such
    line passes, because there is no claim to contradict.
    """
    if not RELEASE_NOTES.exists():
        print(f"  notes: {RELEASE_NOTES.name} is missing; nothing to check")
        return
    quoted = {
        int(match.group(1).replace(",", ""))
        for match in NOTES_WASM_RE.finditer(RELEASE_NOTES.read_text(encoding="utf-8"))
    }
    if not quoted:
        print(f"  notes: no `poly.wasm: <n> bytes` line in {RELEASE_NOTES.name}")
        return
    actual = WASM_MODULE.stat().st_size
    stale = sorted(size for size in quoted if size != actual)
    if stale:
        raise SystemExit(
            f"{RELEASE_NOTES.name} quotes a wasm size the built module does not have: "
            f"notes say {', '.join(f'{size:,}' for size in stale)} bytes, "
            f"{WASM_MODULE.relative_to(ROOT)} is {actual:,} bytes. "
            "Fix the number in the notes, commit it, and re-run with --resume."
        )
    print(f"  notes: wasm size matches the built module ({actual:,} bytes)")


def wasm_size_budget() -> int | None:
    """The byte budget wasm-build.yml gates against, or None if unreadable."""
    match = WASM_BUDGET_RE.search(WASM_WORKFLOW.read_text(encoding="utf-8"))
    return int(match.group(1)) if match else None


def print_wasm_headroom(assets: list[dict[str, object]]) -> None:
    """Report the published module against the budget the gate enforces."""
    asset = next(
        (item for item in assets
         if str(item["name"]).startswith("poly-playground-")
         and str(item["name"]).endswith(".wasm")),
        None,
    )
    if asset is None:
        print("  wasm: no playground module among the published assets")
        return
    size = int(asset["size"])
    budget = wasm_size_budget()
    if budget is None:
        print(f"  wasm: {size:,} bytes (budget not found in {WASM_WORKFLOW.name})")
        return
    headroom = budget - size
    share = size / budget * 100
    state = f"{headroom:,} bytes of headroom" if headroom >= 0 else f"{-headroom:,} bytes OVER budget"
    print(f"  wasm: {size:,} bytes against a {budget:,}-byte budget "
          f"({share:.1f}% used, {state})")


def print_jobs(view: dict[str, object]) -> bool:
    """Print each job's conclusion; True when the run succeeded."""
    jobs = view.get("jobs") or []
    for job in jobs:  # type: ignore[union-attr]
        print(f"    {job['conclusion'] or job['status']:<12} {job['name']}")
    return view.get("conclusion") == "success"


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("version", help="version to release, e.g. 2.0.0-preview.21 (no v prefix)")
    parser.add_argument("--resume", action="store_true",
                        help="the release commit is already pushed (and maybe tagged): "
                             "pick up from the CI check / the release wait")
    parser.add_argument("--skip-tests", action="store_true",
                        help="pass --skip-tests to prepare_release.py")
    parser.add_argument("--dry-run", action="store_true",
                        help="run the read-only checks and print the plan, without changing anything")
    parser.add_argument("--timeout", type=int, default=DEFAULT_TIMEOUT_MINUTES, metavar="MINUTES",
                        help=f"per-run wait limit (default {DEFAULT_TIMEOUT_MINUTES})")
    arguments = parser.parse_args()
    version = arguments.version

    run_started = time.time()
    timings: dict[str, float] = {}

    print("1/7 Preflight")
    if version.startswith("v"):
        raise SystemExit("tags are unprefixed: pass the version without the leading `v`")
    if not VERSION_RE.match(version):
        raise SystemExit(f"not a version: {version!r}")
    run(["gh", "auth", "status"])
    branch = git("rev-parse", "--abbrev-ref", "HEAD").strip()
    print(f"  branch: {branch}")
    if branch in {"master", "main"}:
        raise SystemExit(
            f"refusing to release from {branch}: releases are cut from the default development "
            "branch (v2.0-dev), and master must not receive direct commits"
        )
    head = git("rev-parse", "HEAD").strip()
    tagged: str | None = None
    if tag_exists(version):
        if not arguments.resume:
            raise SystemExit(
                f"tag {version} already exists (locally or on origin); refusing to move it. "
                "If this run was interrupted after tagging, re-run with --resume to wait on the release."
            )
        tagged = tag_commit(version)
        if tagged is None:
            raise SystemExit(f"tag {version} exists but its commit could not be resolved; inspect it by hand")
        if tagged != head:
            raise SystemExit(
                f"tag {version} names {tagged[:9]}, not HEAD ({head[:9]}); a tag on another commit "
                "is a human call, so this run stops here"
            )
        print(f"  HEAD: {head[:9]}, tag {version} already names it: resuming after the tag")
    else:
        print(f"  HEAD: {head[:9]}, tag {version} is free")

    if arguments.dry_run:
        print("\nDry run: nothing was written. A real run would:")
        if tagged:
            print(f"  - check the wasm size in {RELEASE_NOTES.name} against the built module")
            print(f"  - confirm {CI_WORKFLOW} was green at {head[:9]}")
            print(f"  - skip tagging: {version} already names {head[:9]}")
        else:
            if not arguments.resume:
                print(f"  - run scripts/prepare_release.py {version}")
            print(f"  - commit `chore(release): prepare {version}` and push {branch}")
            print(f"  - check the wasm size in {RELEASE_NOTES.name} against the built module")
            print(f"  - wait for the {CI_WORKFLOW} push run at that commit; stop if it is not green")
            print(f"  - `git tag -a {version} -m \"Poly {version}\"` and push the tag")
        print(f"  - wait for the {RELEASE_WORKFLOW} run and print the release")
        return 0

    worktree = changed_paths()
    if arguments.resume:
        if worktree:
            raise SystemExit(
                "--resume expects a clean tree (the release commit is already pushed); "
                f"these paths are modified: {', '.join(worktree)}"
            )
        local_ahead = git("rev-list", "--count", f"origin/{branch}..HEAD").strip()
        if local_ahead != "0":
            raise SystemExit(
                f"HEAD is {local_ahead} commit(s) ahead of origin/{branch}; push it first"
            )
        print("2/7 Prepare: skipped (--resume, HEAD is pushed)")
    else:
        print("2/7 Prepare")
        prepare_started = time.time()
        prepare = [sys.executable, str(PREPARE_SCRIPT), version]
        if arguments.skip_tests:
            prepare.append("--skip-tests")
        run(prepare, allow_failure=False)
        timings["prepare"] = time.time() - prepare_started

        print("3/7 Commit and push")
        changed = changed_paths()
        if not changed:
            print("  no changes: the version was already prepared in this commit")
        else:
            problems = release_surface_problems(changed)
            if problems:
                raise SystemExit(
                    "these changed paths are outside the release surface, so the release commit "
                    "would carry unrelated work; commit or stash them and re-run:\n"
                    + "\n".join(f"  - {path}" for path in problems)
                )
            git("add", "-A")
            git("commit", "-m", f"chore(release): prepare {version}")
        git("push", "origin", branch)
        head = git("rev-parse", "HEAD").strip()

    if arguments.resume:
        print("3/7 Commit and push: skipped")

    print("4/7 Check the release notes")
    check_notes_wasm_size()

    print("5/7 Wait for CI on the release commit")
    ci_started = time.time()
    ci_run = find_run(CI_WORKFLOW, commit=head, branch=branch)
    ci_view = wait_run(ci_run, "CI", arguments.timeout)
    timings["CI wait"] = time.time() - ci_started
    if not print_jobs(ci_view):
        if tagged:
            raise SystemExit(
                f"CI is red for {head[:9]} even though tag {version} already names it; the release "
                "cannot be trusted. Inspect the run, and treat the tag as a human decision."
            )
        raise SystemExit(
            f"CI is red for {head[:9]}, so no tag was created. Fix the cause, commit and push, then "
            f"re-run with --resume:\n  python3 scripts/release.py {version} --resume"
        )
    print(f"  CI is green after {duration(timings['CI wait'])}; the commit is safe to tag")

    if tagged:
        print(f"6/7 Tag and push: skipped ({version} already names {head[:9]})")
    else:
        print("6/7 Tag and push")
        git("tag", "-a", version, "-m", f"Poly {version}")
        git("push", "origin", version)

    print("7/7 Wait for the release")
    release_started = time.time()
    release_run = find_run(RELEASE_WORKFLOW, commit=head, branch=version)
    release_view = wait_run(release_run, "release", arguments.timeout)
    timings["release wait"] = time.time() - release_started
    if not print_jobs(release_view):
        raise SystemExit(
            f"the release run failed for {version}. The release may be published but unverified; "
            f"inspect it with `gh run view {release_run['databaseId']}`, fix the cause, and only then "
            "decide whether to delete the release and re-cut the tag"
        )

    release = gh_json("release", "view", version, "--json",
                      "tagName,isDraft,isPrerelease,url,assets")
    print(f"\nReleased {release['tagName']} (prerelease: {release['isPrerelease']}, "
          f"draft: {release['isDraft']})")
    for asset in release["assets"]:  # type: ignore[index]
        print(f"  {asset['name']}  {asset['size']} bytes")
    print_wasm_headroom(release["assets"])  # type: ignore[arg-type]
    print(f"  {release['url']}")

    print("\nTimings")
    for label, seconds in timings.items():
        print(f"  {label:<14} {duration(seconds)}")
    print(f"  {'total':<14} {duration(time.time() - run_started)}")
    return 0


if __name__ == "__main__":
    try:
        sys.exit(main())
    except KeyboardInterrupt:
        sys.exit(130)
