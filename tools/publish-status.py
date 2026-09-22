#!/usr/bin/env python3
"""Whether a crate version is already on the crates.io index.

This is the decision `.github/workflows/release.yml` makes before each
`cargo publish`: is this exact `name@version` already on the registry, so the
publish should be **skipped**, or is it absent, so the publish should
**proceed**?

Why this exists as a script rather than three lines of shell
------------------------------------------------------------

On 2026-09-21 a release run published five crates and then hit crates.io's
new-crate rate limit on the sixth. Every rerun after that died instantly on
`error: crate conform-core@0.1.0 already exists on crates.io index`, because
the workflow walked the whole sequence and published unconditionally. Five
crates were permanent, eight could not be created, and the release could not
be resumed. A publish loop has to be resumable, and resumable means asking
this question first.

Asking it correctly is fiddlier than it looks, which is the other reason this
is a script: it can be run against the real index and checked.

**It is not done by matching cargo's error text.** `already exists` is a
substring of one specific failure and a plausible substring of others; a loop
that treats "the word appeared" as "safe to continue" swallows genuine
failures and reports a release as complete that is not. The index is the
authority on what is published, so the index is what gets asked.

**A yanked version still counts as published.** `cargo yank` makes a version
un-resolvable for new dependants; it does not free the number, and
republishing over it is refused. So the test is whether the version appears in
the index at all, not whether it appears un-yanked.

**Only an exact version match skips.** A crate whose *name* is on the index
but whose version being released is not has never published that version, and
must still be published. Skipping on the name alone would silently drop a
release.

**A network failure is not an answer.** If the index cannot be consulted this
exits non-zero rather than guessing. Guessing "publish" burns a rate-limit
token on a crate that is already out; guessing "skip" abandons a crate while
reporting success. Neither is recoverable, and both are worse than stopping.

The index layout
----------------

The sparse index at https://index.crates.io serves one file per crate, at a
path derived from the name — and the derivation is *not* uniformly
two-characters-then-two-characters:

    1 char    1/{name}            e.g. 1/a
    2 chars   2/{name}            e.g. 2/bs
    3 chars   3/{first}/{name}    e.g. 3/g/gcc
    4+ chars  {1..2}/{3..4}/{name}  e.g. se/rd/serde, co/nf/conform-core

Getting the short-name cases wrong yields a 404, which reads exactly like "not
published" — a false *publish* for 1-3 character names. Every crate in this
workspace is long enough not to care today, and the day one is not should not
be the day this is discovered. `3/g/gcc` and `gc/c/gcc` were both requested
while writing this: the first is 200, the second is 404.

The file is JSON-lines, one object per version, each carrying `vers`.

Usage
-----

    ./tools/publish-status.py conform-core conform-registry
    ./tools/publish-status.py conform-core@99.0.0

A bare name takes its version from `cargo metadata`, which is the version that
would actually be published. A `name@version` spec states the version outright
and needs no cargo — that form is for exercising this logic against cases the
workspace does not currently contain.

Prints one tab-separated line per spec:

    <decision>\t<name>\t<version>\t<reason>

where `<decision>` is `skip` or `publish`. Exit status is 0 when every spec
was decided, 1 on a usage error, and 2 when the index could not be consulted.
"""

import json
import sys
import time
import urllib.error
import urllib.request

INDEX = "https://index.crates.io"

# crates.io asks that automated clients identify themselves.
USER_AGENT = (
    "open-spec-conform-release-workflow "
    "(https://github.com/OffeneDatenmodellierung/open-spec-conform)"
)

# A transient 5xx or a dropped connection should not stop a release; a
# sustained one must. Three attempts over a few seconds separates the two
# without making a stuck index look like a slow one.
ATTEMPTS = 3
BACKOFF_SECONDS = 2


def index_path(name):
    """The sparse-index path for a crate name.

    Mirrors cargo's own layout, including the 1/2/3-character special cases
    that are not the two-and-two form.
    """
    n = name.lower()
    if len(n) == 1:
        return f"1/{n}"
    if len(n) == 2:
        return f"2/{n}"
    if len(n) == 3:
        return f"3/{n[0]}/{n}"
    return f"{n[:2]}/{n[2:4]}/{n}"


def published_versions(name):
    """Every version of `name` on the index, yanked ones included.

    Returns an empty set when the crate name is not on the index at all.
    Raises RuntimeError when the index could not be consulted, which is a
    different thing from "nothing is published" and is never conflated with
    it.
    """
    url = f"{INDEX}/{index_path(name)}"
    request = urllib.request.Request(url, headers={"User-Agent": USER_AGENT})

    last_error = None
    for attempt in range(1, ATTEMPTS + 1):
        try:
            with urllib.request.urlopen(request, timeout=30) as response:
                body = response.read().decode("utf-8")
            break
        except urllib.error.HTTPError as error:
            # 404 is an answer, not a failure: the name has never been
            # published. Anything else is the index declining to answer.
            if error.code == 404:
                return set()
            last_error = f"HTTP {error.code} from {url}"
        except (urllib.error.URLError, TimeoutError, OSError) as error:
            last_error = f"{type(error).__name__}: {error} ({url})"

        if attempt < ATTEMPTS:
            time.sleep(BACKOFF_SECONDS * attempt)
    else:
        raise RuntimeError(f"could not consult the crates.io index: {last_error}")

    versions = set()
    for line in body.splitlines():
        line = line.strip()
        if not line:
            continue
        try:
            versions.add(json.loads(line)["vers"])
        except (json.JSONDecodeError, KeyError) as error:
            raise RuntimeError(f"unreadable index entry for {name}: {error}") from error
    return versions


def workspace_versions():
    """Each workspace member's declared version, from `cargo metadata`."""
    import subprocess

    result = subprocess.run(
        ["cargo", "metadata", "--no-deps", "--format-version", "1", "--locked"],
        capture_output=True,
        text=True,
        check=True,
    )
    return {p["name"]: p["version"] for p in json.loads(result.stdout)["packages"]}


def decide(name, version):
    """`skip` if this exact version is on the index, `publish` otherwise."""
    versions = published_versions(name)
    if version in versions:
        return "skip", f"{name}@{version} is already on the crates.io index"
    if not versions:
        return "publish", f"{name} is not on the crates.io index"
    known = ", ".join(sorted(versions))
    return (
        "publish",
        f"{name} is on the index but {version} is not (it has: {known})",
    )


def main(argv):
    if not argv:
        print(__doc__, file=sys.stderr)
        return 1

    # `cargo metadata` is only run if some spec actually needs it, so the
    # `name@version` form works anywhere, including outside a checkout.
    declared = None
    specs = []
    for arg in argv:
        if "@" in arg:
            name, _, version = arg.partition("@")
            if not name or not version:
                print(f"::error::malformed spec '{arg}', want name@version", file=sys.stderr)
                return 1
        else:
            if declared is None:
                declared = workspace_versions()
            name = arg
            if name not in declared:
                print(
                    f"::error::'{name}' is not a member of this workspace",
                    file=sys.stderr,
                )
                return 1
            version = declared[name]
        specs.append((name, version))

    for name, version in specs:
        try:
            decision, reason = decide(name, version)
        except RuntimeError as error:
            print(f"::error::{error}", file=sys.stderr)
            return 2
        print(f"{decision}\t{name}\t{version}\t{reason}")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
