# cargo-xwinbuild

[![CI](https://github.com/messense/cargo-xwinbuild/workflows/CI/badge.svg)](https://github.com/messense/cargo-xwinbuild/actions?query=workflow%3ACI)
[![Crates.io](https://img.shields.io/crates/v/cargo-xwinbuild.svg)](https://crates.io/crates/cargo-xwinbuild)
[![docs.rs](https://docs.rs/cargo-xwinbuild/badge.svg)](https://docs.rs/cargo-xwinbuild/)
[![PyPI](https://img.shields.io/pypi/v/cargo-xwinbuild.svg)](https://pypi.org/project/cargo-xwinbuild)

Cross compile Cargo project to Windows msvc target with ease. (LLVM installation required.)

## Installation

```bash
cargo install cargo-xwinbuild
```

You can also install it using pip:

```bash
pip install cargo-xwinbuild
```

## Usage

1. Install [LLVM](https://llvm.org), on macOS: `brew install llvm`
2. Install Rust Windows msvc target via rustup, for example, `rustup target add x86_64-pc-windows-msvc`
3. Run `cargo xwinbuild`, for example, `cargo xwinbuild --target x86_64-pc-windows-msvc`

## License

This work is released under the MIT license. A copy of the license is provided
in the [LICENSE](./LICENSE) file.
