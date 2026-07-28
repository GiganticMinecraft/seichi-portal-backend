"""Read, update, and validate the workspace release version."""

import argparse
import json
import re
import sys
import tomllib
from pathlib import Path


SEMVER = re.compile(r"^(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)$")
WORKSPACE_PACKAGE = re.compile(
    r"(?ms)^\[workspace\.package\][ \t]*\r?\n.*?(?=^\[|\Z)"
)
VERSION_LINE = re.compile(r'(?m)^version[ \t]*=[ \t]*"[^"]+"[ \t]*$')


def parse_semver(version: str) -> tuple[int, int, int]:
    match = SEMVER.fullmatch(version)
    if match is None:
        raise ValueError(f"expected a release SemVer without a pre-release suffix, got {version!r}")
    return tuple(map(int, match.groups()))


def next_version(version: str, bump: str) -> str:
    major, minor, patch = parse_semver(version)
    if bump == "major":
        return f"{major + 1}.0.0"
    if bump == "minor":
        return f"{major}.{minor + 1}.0"
    if bump == "patch":
        return f"{major}.{minor}.{patch + 1}"
    raise ValueError(f"unsupported version bump: {bump}")


def workspace_version(source: str) -> str:
    data = tomllib.loads(source)
    try:
        version = data["workspace"]["package"]["version"]
    except KeyError as error:
        raise ValueError("Cargo.toml has no [workspace.package].version") from error
    if not isinstance(version, str):
        raise ValueError("[workspace.package].version must be a string")
    parse_semver(version)
    return version


def replace_workspace_version(source: str, version: str) -> str:
    parse_semver(version)
    section, replacements = WORKSPACE_PACKAGE.subn(
        lambda match: VERSION_LINE.sub(f'version = "{version}"', match.group(0), count=1),
        source,
        count=1,
    )
    if replacements != 1 or workspace_version(section) != version:
        raise ValueError("could not replace [workspace.package].version exactly once")
    return section


def verify_workspace_versions(metadata: dict[str, object], expected_version: str) -> list[str]:
    parse_semver(expected_version)
    member_ids = set(metadata["workspace_members"])
    packages = {package["id"]: package for package in metadata["packages"]}
    missing = member_ids - packages.keys()
    if missing:
        return [f"workspace member absent from packages: {member}" for member in sorted(missing)]
    return [
        f"{package['name']} has version {package['version']}, expected {expected_version}"
        for member_id in sorted(member_ids)
        if (package := packages[member_id])["version"] != expected_version
    ]


def verify_workspace_inheritance(metadata: dict[str, object], expected_version: str) -> list[str]:
    member_ids = set(metadata["workspace_members"])
    packages = {package["id"]: package for package in metadata["packages"]}
    errors = verify_workspace_versions(metadata, expected_version)
    for member_id in sorted(member_ids & packages.keys()):
        package = packages[member_id]
        manifest_path = Path(package["manifest_path"])
        try:
            declared_version = tomllib.loads(manifest_path.read_text())["package"]["version"]
        except (OSError, KeyError, tomllib.TOMLDecodeError) as error:
            errors.append(f"could not read package version from {manifest_path}: {error}")
            continue
        if declared_version != {"workspace": True}:
            errors.append(f"{manifest_path} must declare version.workspace = true")
    return errors


def main() -> None:
    parser = argparse.ArgumentParser()
    command = parser.add_subparsers(dest="command", required=True)
    read = command.add_parser("read")
    read.add_argument("path")
    bump = command.add_parser("bump")
    bump.add_argument("version")
    bump.add_argument("kind", choices=("patch", "minor", "major"))
    update = command.add_parser("set")
    update.add_argument("path")
    update.add_argument("version")
    verify = command.add_parser("verify-metadata")
    verify.add_argument("version")
    inherited = command.add_parser("verify-workspace")
    inherited.add_argument("version")
    args = parser.parse_args()

    try:
        if args.command == "read":
            source = sys.stdin.read() if args.path == "-" else Path(args.path).read_text()
            print(workspace_version(source))
        elif args.command == "bump":
            print(next_version(args.version, args.kind))
        elif args.command == "set":
            path = Path(args.path)
            path.write_text(replace_workspace_version(path.read_text(), args.version))
        elif args.command == "verify-metadata":
            errors = verify_workspace_versions(json.load(sys.stdin), args.version)
            if errors:
                raise ValueError("\n".join(errors))
        elif args.command == "verify-workspace":
            errors = verify_workspace_inheritance(json.load(sys.stdin), args.version)
            if errors:
                raise ValueError("\n".join(errors))
    except (OSError, ValueError, tomllib.TOMLDecodeError, json.JSONDecodeError) as error:
        print(f"release version error: {error}", file=sys.stderr)
        raise SystemExit(1) from error


if __name__ == "__main__":
    main()
