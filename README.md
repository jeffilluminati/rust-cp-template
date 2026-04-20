# rust-cp-template

Rust competitive programming workspace with a reusable library under `src/`
and multiple per-problem binaries under `src/bin/`.

The codebase is derived from:

- `to-omer/competitive-library` for the algorithm and data structure library
- `Endle/rust_codeforce_template` for the contest-oriented project layout

## Repository layout

- `src/lib.rs`: library entry point exposed as `cp`
- `src/bin/_template.rs`: starter file used when resetting problem files
- `src/bin/{main,a,b,c,d,e,f,g}.rs`: contest or problem entry points
- `cr`: helper python script to bundle, compile, and run a selected binary

## Prerequisites

- Rust toolchain

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

## Useful commands

`./cr --reset` backs up the current problem files under `backup/<timestamp>/`
and restores fresh copies from `src/bin/_template.rs`.

Bundled output is written to `/tmp/`.

## Credits

All credit goes to the amazing [__to-omer__](https://github.com/to-omer) and [__Endle__](https://github.com/Endle) for the library and template they have provided. 

## License

This repository is distributed under the GNU General Public License, version 3.
It includes inherited MIT-licensed and CC0-licensed upstream code.
