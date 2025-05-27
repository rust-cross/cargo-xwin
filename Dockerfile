ARG RUST_VERSION=1.87.0

FROM rust:$RUST_VERSION as builder

ENV CARGO_NET_GIT_FETCH_WITH_CLI=true

# Compile dependencies only for build caching
ADD Cargo.toml /cargo-xwin/Cargo.toml
ADD Cargo.lock /cargo-xwin/Cargo.lock
RUN mkdir /cargo-xwin/src && \
    touch  /cargo-xwin/src/lib.rs && \
    cargo build --manifest-path /cargo-xwin/Cargo.toml --release

# Build cargo-xwin
ADD . /cargo-xwin/
# Manually update the timestamps as ADD keeps the local timestamps and cargo would then believe the cache is fresh
RUN touch /cargo-xwin/src/lib.rs /cargo-xwin/src/bin/cargo-xwin.rs
RUN cargo build --manifest-path /cargo-xwin/Cargo.toml --release

FROM rust:$RUST_VERSION

RUN set -eux; \
    curl --fail https://dl.winehq.org/wine-builds/winehq.key | gpg --dearmor > /usr/share/keyrings/winehq.gpg; \
    echo "deb [signed-by=/usr/share/keyrings/winehq.gpg] https://dl.winehq.org/wine-builds/debian/ bookworm main" > /etc/apt/sources.list.d/winehq.list; \
    # The way the debian package works requires that we add x86 support, even
    # though we are only going be running x86_64 executables. We could also
    # build from source, but that is out of scope.
    dpkg --add-architecture i386; \
    apt-get update && apt-get install --no-install-recommends -y clang llvm winehq-staging cmake ninja-build; \
    apt-get remove -y --auto-remove; \
    rm -rf /var/lib/apt/lists/*;

# Install Rust targets
RUN rustup target add x86_64-pc-windows-msvc aarch64-pc-windows-msvc && \
    rustup component add llvm-tools-preview

COPY --from=builder /cargo-xwin/target/release/cargo-xwin /usr/local/cargo/bin/
