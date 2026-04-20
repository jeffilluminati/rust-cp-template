#!/usr/bin/env python3

# Compile and Run (with rust_bundler_cp)
# MIT LICENSE. Zhenbo Li

import subprocess
import os
import shutil
import sys
import time
from pathlib import Path
import tomllib
import glob
import re
import shlex

RUST_VERSION = "1.94.0" # As of 2025-05-16, Kattis
BUNDLER = "rust_bundler_cp"
STRIP_OUTPUT = ""
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
]


def check_temporary_path():
    return "/tmp"

TEMP_DIRECTORY = check_temporary_path()

BACKUP_DIRECTORY = "backup"
RS_FILE_DIRECTORY = "src/bin/"
TEMPLATE_RS_FILE_NAME = "_template.rs"


def get_time_str():
    return time.strftime("%H_%M_%S", time.localtime())


BUNDLING_TIME = get_time_str()


def check_rust_toolkit():
    # Use WSL to skip it is a bit hacky
    print("Skipped rust version enforce")
    return


def check_valid_cargo_directory():
    x = os.listdir()
    if 'Cargo.toml' not in x:
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


def resolve_binary_path(binary):
    binaries = list_known_binaries()
    return binaries[binary]


def bundle(binary) -> str:
    output_path = f"{TEMP_DIRECTORY}/{binary}"
    command = [BUNDLER, "--input", ".", "--binary", binary]
    if STRIP_OUTPUT:
        command.append(STRIP_OUTPUT)
    command.extend(["--output", output_path + ".rs"])
    env = os.environ.copy()
    env.pop("RUSTC_WRAPPER", None)
    subprocess.run(command, check=True, env=env)
    return output_path


def read_text(path):
    return Path(path).read_text(encoding="utf-8")


def rewrite_edition_specific_syntax(source, edition):
    if edition != "2024":
        return source
    return re.sub(r'(?<!\bunsafe\s)extern\s+"C"', 'unsafe extern "C"', source)


def edition_for_oj(oj):
    normalized = oj.lower()
    if normalized not in OJ_EDITIONS:
        supported = ", ".join(sorted(OJ_EDITIONS))
        print(f"Unknown online judge '{oj}'. Supported values: {supported}")
        exit(1)
    return OJ_EDITIONS[normalized]


def rewrite_submission(rs_file, edition):
    submission = read_text(rs_file + ".rs")
    submission = rewrite_edition_specific_syntax(submission, edition)
    if not submission.endswith("\n"):
        submission += "\n"
    Path(rs_file + ".rs").write_text(submission, encoding="utf-8")


def rustc_release_command(source_path, output_path, edition):
    return ["rustc", f"--edition={edition}", *RELEASE_RUSTC_FLAGS, source_path, "-o", output_path]


def rustc_release_prefix(edition):
    return ["rustc", f"--edition={edition}", *RELEASE_RUSTC_FLAGS]


def compile_rs(rs_file, edition):
    result = subprocess.run(
        rustc_release_command(rs_file + ".rs", rs_file, edition),
        capture_output=True,
        text=True,
    )
    if result.returncode == 0:
        return

    try:
        compile_rs_with_cp_wrapper(rs_file, edition)
        print("Bundled file depends on exported cp macros; compiled via wrapper for local execution")
    except subprocess.CalledProcessError:
        if result.stdout:
            print(result.stdout, end="")
        if result.stderr:
            print(result.stderr, end="", file=sys.stderr)
        raise


def find_compiled_cp():
    for candidate in ("target/debug/libcp.rlib", "target/release/libcp.rlib"):
        if os.path.exists(candidate):
            return candidate

    matches = sorted(glob.glob("target/debug/deps/libcp-*.rlib"))
    if matches:
        return matches[0]

    matches = sorted(glob.glob("target/release/deps/libcp-*.rlib"))
    if matches:
        return matches[0]

    return None


def compile_rs_with_cp_wrapper(rs_file, edition):
    env = os.environ.copy()
    env.pop("RUSTC_WRAPPER", None)
    rustflags = " ".join(RELEASE_RUSTC_FLAGS)
    env["RUSTFLAGS"] = f"{env.get('RUSTFLAGS', '')} {rustflags}".strip()
    cargo_build = subprocess.run(
        ["cargo", "build", "--release", "--lib"],
        check=False,
        capture_output=True,
        text=True,
        env=env,
    )
    if cargo_build.returncode != 0:
        if cargo_build.stdout:
            print(cargo_build.stdout, end="")
        if cargo_build.stderr:
            print(cargo_build.stderr, end="", file=sys.stderr)
        cargo_build.check_returncode()

    cp_rlib = find_compiled_cp()
    if cp_rlib is None:
        print("Failed to locate compiled cp artifact after cargo build --lib")
        exit(1)

    wrapper_path = rs_file + "_wrapper.rs"
    with open(wrapper_path, "w", encoding="utf-8") as fh:
        fh.write("#[macro_use]\n")
        fh.write("extern crate cp;\n\n")
        fh.write(f'include!(r"{rs_file}.rs");\n')

    rustc_command = [
        *rustc_release_prefix(edition),
        "--crate-name",
        Path(rs_file).name + "_wrapper",
        wrapper_path,
        "-L",
        "dependency=target/release/deps",
        "--extern",
        f"cp={cp_rlib}",
        "-o",
        rs_file,
    ]
    wrapper_compile = subprocess.run(
        rustc_command,
        check=False,
        capture_output=True,
        text=True,
        env=env,
    )
    if wrapper_compile.returncode != 0:
        if wrapper_compile.stdout:
            print(wrapper_compile.stdout, end="")
        if wrapper_compile.stderr:
            print(wrapper_compile.stderr, end="", file=sys.stderr)
        wrapper_compile.check_returncode()


def run_with_timing(binary_path):
    command = f"time -p {shlex.quote(binary_path)}"
    subprocess.run(["bash", "-c", command], check=True)


def reset_workspace():
    backup_dir = BACKUP_DIRECTORY + "/" + BUNDLING_TIME + "/"
    subprocess.run(["mkdir", "-p", BACKUP_DIRECTORY])
    subprocess.run(["mkdir", "-p", backup_dir])
    for filename in os.listdir(RS_FILE_DIRECTORY):
        if not filename.endswith("rs"):
            continue
        if filename == TEMPLATE_RS_FILE_NAME:
            continue
        subprocess.run(["mv", RS_FILE_DIRECTORY+filename, backup_dir + filename])
        subprocess.run(["cp", RS_FILE_DIRECTORY + TEMPLATE_RS_FILE_NAME, RS_FILE_DIRECTORY+filename])
    print("Previous result code backed up tp " + backup_dir)
    exit(0)


def bundler_supports(flag):
    result = subprocess.run(
        [BUNDLER, "--help"],
        check=False,
        capture_output=True,
        text=True,
    )
    return result.returncode == 0 and flag in result.stdout


def resolve_bundler():
    bleed = "./" + BUNDLER
    if os.path.exists(bleed):
        print("Using bleeding edge bundler")
        return bleed

    resolved = shutil.which(BUNDLER)
    if resolved is not None:
        return resolved

    cargo_home = Path(os.environ.get("CARGO_HOME", Path.home() / ".cargo"))
    cargo_bin = cargo_home / "bin" / BUNDLER
    if cargo_bin.exists():
        return str(cargo_bin)

    print(f"Failed to locate bundler '{BUNDLER}' in PATH or {cargo_bin}")
    exit(1)


def configure_bundler():
    global BUNDLER
    global STRIP_OUTPUT
    BUNDLER = resolve_bundler()

    if bundler_supports("--remove_unused_mod"):
        STRIP_OUTPUT = "--remove_unused_mod"


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
    configure_bundler()

    binary_arg, edition = parse_args(sys.argv[1:])
    binary = resolve_binary(binary_arg or "rust_codeforce_template")
    rs_file = bundle(binary)
    rewrite_submission(rs_file, edition)
    compile_rs(rs_file, edition)
    run_with_timing(rs_file)

if __name__ == "__main__":
    main()
