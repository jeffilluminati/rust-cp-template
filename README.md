# rust-cp-template

Rust competitive programming workspace with a reusable library under `src/`
and multiple per-problem binaries under `src/bin/`.

The codebase is derived from:

- `to-omer/competitive-library` for the algorithm and data structure library
- `Endle/rust_codeforce_template` for the contest-oriented project layout

## Repository layout

- `src/lib.rs`: library entry point exposed as `my_lib`
- `src/bin/_template.rs`: starter file used when resetting problem files
- `src/bin/{main,a,b,c,d,e,f,g}.rs`: contest or problem entry points
- `cr.py`: bundle, compile, and run a selected binary with `rust_bundler_cp`
- `compile_bleeding_edge_bundler.sh`: build a local `rust_bundler_cp` binary
- `run_with_bundle.sh`: troubleshooting helper for bundled output

## Prerequisites

- Rust toolchain
- `rust_bundler_cp`

Install the bundler from crates.io:

```bash
cargo install rust_bundler_cp
```

If you want to test against a local checkout of the bundler, place the
`rust-bundler-cp` repository next to this one and run:

```bash
./compile_bleeding_edge_bundler.sh
```

When a local `./rust_bundler_cp` binary exists, `cr.py` will prefer it over
the globally installed version.

## Typical workflow

1. Edit the target binary, for example `src/bin/a.rs` or `src/bin/main.rs`.
2. Run it normally during development:

```bash
cargo run --bin a
```

3. Bundle, compile, and execute the single-file submission:

```bash
./cr a
```

If no binary name is provided, `cr.py` defaults to `rust_codeforce_template`.

## Useful commands

```bash
cargo test
cargo run --bin main
./cr main
./cr --reset
```

`./cr --reset` backs up the current problem files under `backup/<timestamp>/`
and restores fresh copies from `src/bin/_template.rs`.

Bundled output is written to `/tmp/`.

## Credits

All credit goes to the amazing [__to-omer__](https://github.com/to-omer) and [__Endle__](https://github.com/Endle) for the amazing code and template they have provided. 

## License

This repository is distributed under the GNU General Public License, version 3.
It includes inherited MIT-licensed upstream code, and the original MIT notice
is preserved in [LICENSE](LICENSE).
