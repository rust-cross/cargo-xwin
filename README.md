# cargo-xwin

_formerly cargo-xwinbuild_

[![CI](https://github.com/rust-cross/cargo-xwin/workflows/CI/badge.svg)](https://github.com/rust-cross/cargo-xwin/actions?query=workflow%3ACI)
[![Crates.io](https://img.shields.io/crates/v/cargo-xwin.svg)](https://crates.io/crates/cargo-xwin)
[![docs.rs](https://docs.rs/cargo-xwin/badge.svg)](https://docs.rs/cargo-xwin/)
[![PyPI](https://img.shields.io/pypi/v/cargo-xwin.svg)](https://pypi.org/project/cargo-xwin)
[![Docker Image](https://img.shields.io/docker/pulls/messense/cargo-xwin.svg?maxAge=2592000)](https://hub.docker.com/r/messense/cargo-xwin/)

> 🚀 Help me to become a full-time open-source developer by [sponsoring me on GitHub](https://github.com/sponsors/messense)

Cross compile Cargo project to Windows msvc target with ease using [xwin](https://github.com/Jake-Shadle/xwin) or [windows-msvc-sysroot](https://github.com/trcrsired/windows-msvc-sysroot).

**By using this software you are consented to accept the license at [https://go.microsoft.com/fwlink/?LinkId=2086102](https://go.microsoft.com/fwlink/?LinkId=2086102)**

## Prerequisite

1. Install [clang](https://clang.llvm.org/) (On macOS run `brew install llvm` and you're good to go).
2. For assembly dependencies, install `llvm-tools` component via `rustup component add llvm-tools` or install [llvm](https://llvm.org).

A full LLVM installation is recommended to avoid possible issues.

## Installation

```bash
cargo install --locked cargo-xwin
```

You can also install it using pip:

```bash
pip install cargo-xwin
```

We also provide a [Docker image](https://hub.docker.com/r/messense/cargo-xwin) which has wine pre-installed in addition to cargo-xwin and Rust,
for example to build for x86_64 Windows:

```bash
docker run --rm -it -v $(pwd):/io -w /io messense/cargo-xwin \
  cargo xwin build --release --target x86_64-pc-windows-msvc
```

## Usage

1. Install Rust Windows msvc target via rustup, for example, `rustup target add x86_64-pc-windows-msvc`
2. Run `cargo xwin build`, for example, `cargo xwin build --target x86_64-pc-windows-msvc`

### Run tests with wine

With wine installed, you can run tests with the `cargo xwin test` command,
for example, `cargo xwin test --target x86_64-pc-windows-msvc`

### Customization

The Microsoft CRT and Windows SDK can be customized using the following environment variables or CLI options.

| Environment Variable         | CLI option                     | Description                                                                                                        |
| ---------------------------- | ------------------------------ | ------------------------------------------------------------------------------------------------------------------ |
| `XWIN_CROSS_COMPILER`        | `--cross-compiler`             | The cross compiler to use, defaults to `clang-cl`, possible values: `clang-cl`, `clang`                            |
| `XWIN_ARCH`                  | `--xwin-arch`                  | The architectures to include, defaults to `x86_64,aarch64`, possible values: x86, x86_64, aarch, aarch64           |
| `XWIN_VARIANT`               | `--xwin-variant`               | The variants to include, defaults to `desktop`, possible values: desktop, onecore, spectre                         |
| `XWIN_VERSION`               | `--xwin-version`               | The version to retrieve, defaults to 16, can either be a major version of 15 or 16, or a `<major>.<minor>` version |
| `XWIN_SDK_VERSION`           | `--xwin-sdk-version`           | The SDK version to retrieve, defaults to the latest version                                                        |
| `XWIN_CRT_VERSION`           | `--xwin-crt-version`           | The CRT version to retrieve, defaults to the latest version                                                        |
| `XWIN_INCLUDE_ATL`           | `--xwin-include-atl`           | Whether to include the Active Template Library (ATL) in the installation                                           |
| `XWIN_CACHE_DIR`             | `--xwin-cache-dir`             | xwin cache directory to put CRT and SDK files                                                                      |
| `XWIN_INCLUDE_DEBUG_LIBS`    | `--xwin-include-debug-libs`    | Whether or not to include debug libs in installation (default false).                                              |
| `XWIN_INCLUDE_DEBUG_SYMBOLS` | `--xwin-include-debug-symbols` | Whether or not to include debug symbols (PDBs) in installation (default false).                                    |

### CMake Support

Some Rust crates use the [cmake](https://github.com/alexcrichton/cmake-rs) crate to build C/C++ dependencies,
cargo-xwin will generate a [CMake toolchain](https://cmake.org/cmake/help/latest/manual/cmake-toolchains.7.html) file
automatically to make cross compilation work out of the box.

**[ninja](https://ninja-build.org/) is required** to enable CMake support.

## License

This work is released under the MIT license. A copy of the license is provided
in the [LICENSE](./LICENSE) file.
