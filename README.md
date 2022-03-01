# cargo-xwinbuild

[![CI](https://github.com/messense/cargo-xwinbuild/workflows/CI/badge.svg)](https://github.com/messense/cargo-xwinbuild/actions?query=workflow%3ACI)
[![Crates.io](https://img.shields.io/crates/v/cargo-xwinbuild.svg)](https://crates.io/crates/cargo-xwinbuild)
[![docs.rs](https://docs.rs/cargo-xwinbuild/badge.svg)](https://docs.rs/cargo-xwinbuild/)
[![PyPI](https://img.shields.io/pypi/v/cargo-xwinbuild.svg)](https://pypi.org/project/cargo-xwinbuild)

Cross compile Cargo project to Windows msvc target with ease. (LLVM installation required.)

**By using this software you are consented to accept the license at [https://go.microsoft.com/fwlink/?LinkId=2086102](https://go.microsoft.com/fwlink/?LinkId=2086102)**

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

## Customization

The Microsoft CRT and Windows SDK can be customized using the following environment variables or CLI options.

| Environment Variable | CLI option         | Description                                                                                                        |
|----------------------|--------------------|--------------------------------------------------------------------------------------------------------------------|
| `XWIN_ARCH`          | `--xwin-arch`      | The architectures to include, defaults to `x86_64,aarch64`, possible values: x86, x86_64, aarch, aarch64           |
| `XWIN_VARIANT`       | `--xwin-variant`   | The variants to include, defaults to `desktop`, possible values: desktop, onecore, spectre                         |
| `XWIN_VERSION`       | `--xwin-version`   | The version to retrieve, defaults to 16, can either be a major version of 15 or 16, or a `<major>.<minor>` version |
| `XWIN_CACHE_DIR`     | `--xwin-cache-dir` | xwin cache directory to put CRT and SDK files                                                                      |

## License

This work is released under the MIT license. A copy of the license is provided
in the [LICENSE](./LICENSE) file.
