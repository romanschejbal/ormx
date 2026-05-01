#!/usr/bin/env python3
"""
Bump the workspace version in the root Cargo.toml.

Edits two locations:
  (a) [workspace.package] version = "<cur>"  -> "<new>"
  (b) the five internal path+version deps in [workspace.dependencies]
      (ferriorm-core, -parser, -codegen, -migrate, -runtime).

Reads the current version from [workspace.package] so the caller only
has to supply the new version. Errors out if either edit doesn't match
the expected number of substitutions, so we never silently mis-bump.

Usage:
    scripts/bump-version.py <new_version>
"""

import pathlib
import re
import sys

SEMVER_RE = re.compile(r"^[0-9]+\.[0-9]+\.[0-9]+(-[a-zA-Z0-9.-]+)?$")


def main() -> int:
    if len(sys.argv) != 2:
        print("usage: bump-version.py <new_version>", file=sys.stderr)
        return 2

    new = sys.argv[1]
    if not SEMVER_RE.match(new):
        print(f"error: '{new}' is not a valid semver version", file=sys.stderr)
        return 2

    path = pathlib.Path("Cargo.toml")
    text = path.read_text()

    cur_match = re.search(
        r'\[workspace\.package\][^\[]*?version = "([^"]+)"',
        text,
        re.DOTALL,
    )
    if not cur_match:
        print("error: could not read current workspace version", file=sys.stderr)
        return 1
    cur = cur_match.group(1)

    if cur == new:
        print(f"error: Cargo.toml is already at {new}; nothing to bump", file=sys.stderr)
        return 1

    pattern_pkg = re.compile(
        r'(\[workspace\.package\][^\[]*?version = ")' + re.escape(cur) + r'(")',
        re.DOTALL,
    )
    text, n_pkg = pattern_pkg.subn(r"\g<1>" + new + r"\g<2>", text, count=1)
    if n_pkg != 1:
        print("error: failed to bump workspace.package.version", file=sys.stderr)
        return 1

    pattern_dep = re.compile(
        r'(ferriorm-[a-z]+ = \{ path = "crates/ferriorm-[a-z]+", version = ")'
        + re.escape(cur)
        + r'(")'
    )
    text, n_dep = pattern_dep.subn(r"\g<1>" + new + r"\g<2>", text)
    if n_dep != 5:
        print(
            f"error: expected 5 internal dep version bumps, got {n_dep}",
            file=sys.stderr,
        )
        return 1

    path.write_text(text)
    print(f"bumped {cur} -> {new} (workspace.package + {n_dep} internal deps)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
