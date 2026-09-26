#!/usr/bin/env python3
import argparse
import pathlib
import re
import sys

DEFAULT_LIMIT = 1_000_000


def read_level_name(root: pathlib.Path) -> str:
    properties = root / "server.properties"
    if not properties.is_file():
        return "world"
    for raw in properties.read_text(encoding="utf-8", errors="replace").splitlines():
        line = raw.strip()
        if line.startswith("level-name="):
            value = line.split("=", 1)[1].strip()
            return value or "world"
    return "world"


def rewrite(path: pathlib.Path, replacements: list[tuple[re.Pattern[str], int]], dry_run: bool) -> tuple[bool, list[str]]:
    if not path.is_file():
        return False, [f"skip missing {path}"]

    original = path.read_text(encoding="utf-8")
    updated = original
    messages: list[str] = []

    for pattern, value in replacements:
        def repl(match: re.Match[str]) -> str:
            old = match.group("value")
            if old != str(value):
                messages.append(f"{path}: {match.group('key')} {old} -> {value}")
            return f"{match.group('prefix')}{value}{match.group('suffix')}"

        updated, count = pattern.subn(repl, updated)
        if count == 0:
            messages.append(f"warning: key not found in {path}: {pattern.pattern}")

    changed = updated != original
    if changed and not dry_run:
        path.write_text(updated, encoding="utf-8")
    return changed, messages


def key_pattern(key: str) -> re.Pattern[str]:
    return re.compile(
        rf"^(?P<prefix>\s*(?P<key>{re.escape(key)})\s*:\s*)(?P<value>-?\d+)(?P<suffix>\s*)$",
        re.MULTILINE,
    )


def main() -> int:
    parser = argparse.ArgumentParser(description="Apply NsTut FTB Chunks claim/force-load limits without rewriting unrelated SNBT settings.")
    parser.add_argument("root", type=pathlib.Path, help="TFG server root")
    parser.add_argument("--claimed", type=int, default=DEFAULT_LIMIT)
    parser.add_argument("--force-loaded", type=int, default=DEFAULT_LIMIT)
    parser.add_argument("--dry-run", action="store_true")
    args = parser.parse_args()

    if args.claimed < 0 or args.force_loaded < 0:
        parser.error("limits must be non-negative")

    root = args.root.resolve()
    level_name = read_level_name(root)
    world = (root / level_name).resolve()
    try:
        world.relative_to(root)
    except ValueError:
        print(f"refusing level-name outside server root: {level_name!r}", file=sys.stderr)
        return 2
    serverconfig = world / "serverconfig"

    changed_any = False
    all_messages: list[str] = []

    changed, messages = rewrite(
        serverconfig / "ftbchunks-world.snbt",
        [
            (key_pattern("max_claimed_chunks"), args.claimed),
            (key_pattern("max_force_loaded_chunks"), args.force_loaded),
        ],
        args.dry_run,
    )
    changed_any |= changed
    all_messages.extend(messages)

    changed, messages = rewrite(
        serverconfig / "ftbranks" / "ranks.snbt",
        [
            (key_pattern("ftbchunks.max_claimed"), args.claimed),
            (key_pattern("ftbchunks.max_force_loaded"), args.force_loaded),
        ],
        args.dry_run,
    )
    changed_any |= changed
    all_messages.extend(messages)

    mode = "plan" if args.dry_run else "apply"
    print(f"FTB chunk-limit {mode}: level-name={level_name!r}, claimed={args.claimed}, force-loaded={args.force_loaded}")
    for message in all_messages:
        print(message)
    if not changed_any:
        print("FTB chunk limits already match desired values (or config files are not present yet).")
    return 0


if __name__ == "__main__":
    sys.exit(main())
