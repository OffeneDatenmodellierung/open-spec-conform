#!/usr/bin/env python3
"""The order this workspace's crates can be published to crates.io in.

Read from `cargo metadata`, never from a list written down here.

A crate cannot be published until every crate it depends on is already on the
registry, because `cargo publish` runs a verification build of the extracted
tarball and that build resolves path dependencies from crates.io rather than
from this workspace. So the sequence is a topological sort of the workspace's
internal dependency graph, and getting it wrong means a publish that fails
half way with the earlier crates already permanent.

Why this is computed rather than listed: four crates arrived in this workspace
between one commit and the next, and a sequence written down in a workflow
file would have gone on publishing the other nine while looking complete.
`.github/workflows/release.yml` calls this, and `docs/PUBLISHING.md` shows
what it prints today rather than asserting an order of its own.

Dev-dependencies count. A dev-dependency on a workspace sibling must also be
on the registry, because the verification build compiles the test targets.

Prints one line: the publishable crates, in an order that is always safe.
Crates carrying `publish = false` are absent — they are not published at all.
"""

import json, subprocess, sys

meta = json.loads(subprocess.run(
    ["cargo", "metadata", "--no-deps", "--format-version", "1", "--locked"],
    capture_output=True, text=True, check=True).stdout)

members = {p["name"]: p for p in meta["packages"]}
# `publish: []` is `publish = false`; `null` is the default, which is "yes".
publishable = {n for n, p in members.items() if p.get("publish") != []}

edges = {}
for name in publishable:
    deps = set()
    for d in members[name]["dependencies"]:
        # Every kind counts: a dev-dependency must also be on the registry,
        # because `cargo publish` builds the test targets during verification.
        if d["name"] in publishable and d["name"] != name:
            deps.add(d["name"])
    edges[name] = deps

order, placed = [], set()
while len(placed) < len(publishable):
    ready = sorted(n for n in publishable if n not in placed and edges[n] <= placed)
    if not ready:
        print("cycle among:", sorted(publishable - placed), file=sys.stderr)
        sys.exit(1)
    order.extend(ready)
    placed.update(ready)

print(" ".join(order))
