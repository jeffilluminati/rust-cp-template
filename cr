#!/usr/bin/env python3

# Compile and Run (with rust_bundler_cp)
# MIT LICENSE. Zhenbo Li

import os
import shlex
import shutil
import subprocess
import sys
import time
from pathlib import Path

import tomllib

BUNDLER_NAME = "rust_bundler_cp"
DEFAULT_OJ = "codeforces"
OJ_EDITIONS = {
    "cses": "2021",
    "codeforces": "2024",
    "atcoder": "2024",
    "kattis": "2024",
}
RELEASE_RUSTC_FLAGS = [
    "-C",
    "opt-level=2",
    "-C",
    "codegen-units=16",
    "-A",
    "warnings",
    "-A",
    "macro_expanded_macro_exports_accessed_by_absolute_paths",
]
TEMP_DIRECTORY = Path("/tmp")
BACKUP_DIRECTORY = Path("backup")
RS_FILE_DIRECTORY = Path("src/bin")
TEMPLATE_RS_FILE_NAME = "_template.rs"


def get_time_str():
    return time.strftime("%H_%M_%S", time.localtime())


BUNDLING_TIME = get_time_str()


def check_rust_toolkit():
    # Use WSL to skip it is a bit hacky
    print("Skipped rust version enforce")
    return


def check_valid_cargo_directory():
    if not Path("Cargo.toml").exists():
        print("Not a cargo project. Aborting")
        exit(1)


def list_known_binaries():
    binaries = {}
    cargo_toml = Path("Cargo.toml")

    with cargo_toml.open("rb") as fh:
        cargo = tomllib.load(fh)

    for bin_entry in cargo.get("bin", []):
        name = bin_entry.get("name")
        path = bin_entry.get("path")
        if not name or not path:
            continue
        binaries[name] = path

    bin_dir = Path(RS_FILE_DIRECTORY)
    if bin_dir.exists():
        for path in bin_dir.glob("*.rs"):
            if path.name == TEMPLATE_RS_FILE_NAME:
                continue
            binaries.setdefault(path.stem, str(path))

    return binaries


def resolve_binary(binary_arg):
    binaries = list_known_binaries()
    if binary_arg in binaries:
        return binary_arg

    candidate = Path(binary_arg)
    normalized_name = candidate.stem if candidate.suffix == ".rs" else candidate.name
    if normalized_name in binaries:
        return normalized_name

    normalized_path = str(candidate)
    for name, path in binaries.items():
        if normalized_path == path:
            return name
        if normalized_path == f"./{path}":
            return name

    available = ", ".join(sorted(binaries))
    print(f"Unknown binary '{binary_arg}'. Available binaries: {available}")
    exit(1)
def bundle(bundler, binary, edition) -> Path:
    output_path = TEMP_DIRECTORY / binary
    command = [bundler, "--input", ".", "--binary", binary, "--edition", edition]
    command.extend(["--output", str(output_path) + ".rs"])
    env = os.environ.copy()
    env.pop("RUSTC_WRAPPER", None)
    subprocess.run(command, check=True, env=env)
    return output_path


def edition_for_oj(oj):
    normalized = oj.lower()
    if normalized not in OJ_EDITIONS:
        supported = ", ".join(sorted(OJ_EDITIONS))
        print(f"Unknown online judge '{oj}'. Supported values: {supported}")
        exit(1)
    return OJ_EDITIONS[normalized]


def rustc_release_command(source_path, output_path, edition):
    return ["rustc", f"--edition={edition}", *RELEASE_RUSTC_FLAGS, source_path, "-o", output_path]


def compile_rs(rs_file, edition):
    rs_source = str(rs_file) + ".rs"
    result = subprocess.run(
        rustc_release_command(rs_source, str(rs_file), edition),
        capture_output=True,
        text=True,
    )
    if result.stdout:
        print(result.stdout, end="")
    if result.stderr:
        print(result.stderr, end="", file=sys.stderr)
    result.check_returncode()


def run_with_timing(binary_path):
    command = f"time -p {shlex.quote(str(binary_path))}"
    subprocess.run(["bash", "-c", command], check=True)


def reset_workspace():
    backup_dir = BACKUP_DIRECTORY / BUNDLING_TIME
    BACKUP_DIRECTORY.mkdir(parents=True, exist_ok=True)
    backup_dir.mkdir(parents=True, exist_ok=True)
    for source_path in RS_FILE_DIRECTORY.iterdir():
        if source_path.suffix != ".rs":
            continue
        if source_path.name == TEMPLATE_RS_FILE_NAME:
            continue
        backup_path = backup_dir / source_path.name
        source_path.rename(backup_path)
        shutil.copyfile(RS_FILE_DIRECTORY / TEMPLATE_RS_FILE_NAME, source_path)
    print(f"Previous result code backed up to {backup_dir}/")
    exit(0)


def resolve_bundler():
    bleed = "./" + BUNDLER_NAME
    if os.path.exists(bleed):
        print("Using bleeding edge bundler")
        return bleed

    resolved = shutil.which(BUNDLER_NAME)
    if resolved is not None:
        return resolved

    cargo_bin = Path(os.environ.get("CARGO_HOME", Path.home() / ".cargo")) / "bin" / BUNDLER_NAME
    if cargo_bin.exists():
        return str(cargo_bin)

    print(f"Failed to locate bundler '{BUNDLER_NAME}' in PATH or {cargo_bin}")
    exit(1)


def parse_args(argv):
    oj = DEFAULT_OJ
    binary = None
    i = 0

    while i < len(argv):
        arg = argv[i]
        if arg == "--reset":
            reset_workspace()
        elif arg in ("--oj", "--judge"):
            i += 1
            if i >= len(argv):
                print(f"Missing value for {arg}")
                exit(1)
            oj = argv[i]
        elif arg.startswith("--oj="):
            oj = arg.split("=", 1)[1]
            if not oj:
                print("Missing value for --oj")
                exit(1)
        elif arg.startswith("--judge="):
            oj = arg.split("=", 1)[1]
            if not oj:
                print("Missing value for --judge")
                exit(1)
        elif arg.startswith("--"):
            print(f"Unknown option '{arg}'")
            exit(1)
        elif binary is None:
            binary = arg
        else:
            print(f"Unexpected argument '{arg}'")
            exit(1)
        i += 1

    return binary, edition_for_oj(oj)


def main():
    check_rust_toolkit()
    check_valid_cargo_directory()
    bundler = resolve_bundler()

    binary_arg, edition = parse_args(sys.argv[1:])
    binary = resolve_binary(binary_arg or "rust_codeforce_template")
    rs_file = bundle(bundler, binary, edition)
    compile_rs(rs_file, edition)
    run_with_timing(rs_file)

if __name__ == "__main__":
    main()
