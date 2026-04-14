#!/usr/bin/env python3

# Compile and Run (with rust_bundler_cp)
# MIT LICENSE. Zhenbo Li

import subprocess
import os
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
BLEEDING = False
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


def strip_from_marker(text, marker):
    idx = text.find(marker)
    if idx == -1:
        return text
    return text[:idx]


def normalize_entry_source(source):
    return source.replace("my_lib::", "crate::")


def find_rust_block_end(text, start_idx):
    brace_idx = text.find("{", start_idx)
    if brace_idx == -1:
        return None

    depth = 0
    i = brace_idx
    in_string = False
    in_char = False
    in_line_comment = False
    in_block_comment = False
    escape = False

    while i < len(text):
        ch = text[i]
        nxt = text[i + 1] if i + 1 < len(text) else ""

        if in_line_comment:
            if ch == "\n":
                in_line_comment = False
            i += 1
            continue

        if in_block_comment:
            if ch == "*" and nxt == "/":
                in_block_comment = False
                i += 2
                continue
            i += 1
            continue

        if in_string:
            if escape:
                escape = False
            elif ch == "\\":
                escape = True
            elif ch == '"':
                in_string = False
            i += 1
            continue

        if in_char:
            if escape:
                escape = False
            elif ch == "\\":
                escape = True
            elif ch == "'":
                in_char = False
            i += 1
            continue

        if ch == "/" and nxt == "/":
            in_line_comment = True
            i += 2
            continue
        if ch == "/" and nxt == "*":
            in_block_comment = True
            i += 2
            continue
        if ch == '"':
            in_string = True
            i += 1
            continue
        if ch == "'":
            in_char = True
            i += 1
            continue

        if ch == "{":
            depth += 1
        elif ch == "}":
            depth -= 1
            if depth == 0:
                return i + 1
        i += 1

    return None


def extract_bundled_support(bundle_text):
    solve_match = re.search(r"\b(?:pub\s+)?fn\s+solve\s*\(", bundle_text)
    if not solve_match:
        return bundle_text.strip()

    main_matches = list(re.finditer(r"(?:\b(?:crate::)?main!\s*\([^;]*\)\s*;)", bundle_text, re.S))
    if main_matches:
        support = (bundle_text[:solve_match.start()] + "\n" + bundle_text[main_matches[-1].end():]).strip()
        return support

    main_fn_match = re.search(r"\bfn\s+main\s*\(", bundle_text[solve_match.start():])
    if main_fn_match:
        main_start = solve_match.start() + main_fn_match.start()
        main_end = find_rust_block_end(bundle_text, main_start)
        if main_end is not None:
            support = (bundle_text[:solve_match.start()] + "\n" + bundle_text[main_end:]).strip()
            return support

    return bundle_text.strip()


def compact_rust_section(text):
    compact_lines = []
    for line in text.splitlines():
        stripped = line.strip()
        if not stripped:
            continue
        if stripped.startswith("//"):
            continue
        compact_lines.append(stripped)
    return " ".join(compact_lines)


def build_submission_boilerplate():
    array_text = strip_from_marker(read_text("src/tools/array.rs"), "\n#[test]").strip()
    scanner_text = strip_from_marker(read_text("src/tools/scanner.rs"), "\n#[cfg(test)]").strip()
    iter_print_text = strip_from_marker(read_text("src/tools/iter_print.rs"), "\n#[cfg(test)]").strip()

    main_text = read_text("src/tools/main.rs")
    imports_start = main_text.find("#[allow(unused_imports)]")
    macros_start = main_text.find("mod main_macros {")
    if imports_start == -1 or macros_start == -1:
        print("Failed to extract submission boilerplate from src/tools/main.rs")
        exit(1)

    imports_text = main_text[imports_start:macros_start].strip()
    macros_text = main_text[macros_start:].strip()
    macros_text = macros_text.replace(
        "$crate::tools::read_stdin_all_unchecked()",
        "$crate::read_stdin_all_unchecked()",
    )
    macros_text = macros_text.replace(
        "$crate::tools::read_stdin_line()",
        "$crate::read_stdin_line()",
    )
    macros_text = macros_text.replace(
        "$crate::tools::Scanner::new",
        "$crate::Scanner::new",
    )

    return "\n\n".join([array_text, scanner_text, iter_print_text, imports_text, macros_text])


def rewrite_submission(binary, rs_file):
    binary_source = read_text(resolve_binary_path(binary))
    bundled_source = read_text(rs_file + ".rs")
    bundled_support = extract_bundled_support(bundled_source)

    sections = [normalize_entry_source(binary_source).strip()]
    if bundled_support:
        sections.append(compact_rust_section(bundled_support))
    sections.append(compact_rust_section(build_submission_boilerplate()))

    submission = "\n\n".join(section for section in sections if section) + "\n"
    Path(rs_file + ".rs").write_text(submission, encoding="utf-8")


def rustc_release_command(source_path, output_path):
    return ["rustc", "--edition=2024", *RELEASE_RUSTC_FLAGS, source_path, "-o", output_path]


def rustc_release_prefix():
    return ["rustc", "--edition=2024", *RELEASE_RUSTC_FLAGS]


def compile_rs(rs_file):
    result = subprocess.run(
        rustc_release_command(rs_file + ".rs", rs_file),
        capture_output=True,
        text=True,
    )
    if result.returncode == 0:
        return

    try:
        compile_rs_with_my_lib_wrapper(rs_file)
        print("Bundled file depends on exported my_lib macros; compiled via wrapper for local execution")
    except subprocess.CalledProcessError:
        if result.stdout:
            print(result.stdout, end="")
        if result.stderr:
            print(result.stderr, end="", file=sys.stderr)
        raise


def find_compiled_my_lib():
    for candidate in ("target/debug/libmy_lib.rlib", "target/release/libmy_lib.rlib"):
        if os.path.exists(candidate):
            return candidate

    matches = sorted(glob.glob("target/debug/deps/libmy_lib-*.rlib"))
    if matches:
        return matches[0]

    matches = sorted(glob.glob("target/release/deps/libmy_lib-*.rlib"))
    if matches:
        return matches[0]

    return None


def compile_rs_with_my_lib_wrapper(rs_file):
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

    my_lib_rlib = find_compiled_my_lib()
    if my_lib_rlib is None:
        print("Failed to locate compiled my_lib artifact after cargo build --lib")
        exit(1)

    wrapper_path = rs_file + "_wrapper.rs"
    with open(wrapper_path, "w", encoding="utf-8") as fh:
        fh.write("#[macro_use]\n")
        fh.write("extern crate my_lib;\n\n")
        fh.write(f'include!(r"{rs_file}.rs");\n')

    rustc_command = [
        *rustc_release_prefix(),
        "--crate-name",
        Path(rs_file).name + "_wrapper",
        wrapper_path,
        "-L",
        "dependency=target/release/deps",
        "--extern",
        f"my_lib={my_lib_rlib}",
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


def check_bleeding_edge_bundler():
    global BUNDLER
    global BLEEDING
    global STRIP_OUTPUT
    bleed = "./" + BUNDLER
    if os.path.exists(bleed):
        print("Using bleeding edge bundler")
        BUNDLER = bleed
        BLEEDING = True
        STRIP_OUTPUT = "--remove_unused_mod"


def main():
    check_rust_toolkit()
    check_valid_cargo_directory()
    check_bleeding_edge_bundler()

    binary = "rust_codeforce_template"

    if "--reset" in sys.argv:
        reset_workspace()

    if len(sys.argv) >= 2:
        binary = resolve_binary(sys.argv[1])
    else:
        binary = resolve_binary(binary)
    rs_file = bundle(binary)
    rewrite_submission(binary, rs_file)
    compile_rs(rs_file)
    run_with_timing(rs_file)

if __name__ == "__main__":
    main()
