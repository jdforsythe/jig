default:
    just --list

build:
    cargo build --workspace

build-headless:
    cargo build --profile release-headless --no-default-features -p jig-cli

test:
    cargo test --workspace

lint:
    cargo clippy --workspace -- -D warnings

check:
    cargo check --workspace

fmt:
    cargo fmt --all

fmt-check:
    cargo fmt --all -- --check

bloat:
    cargo bloat --release --no-default-features -p jig-cli

audit:
    cargo audit

size-gate:
    #!/usr/bin/env bash
    just build-headless
    SIZE=$(stat -f%z target/release-headless/jig 2>/dev/null || stat -c%s target/release-headless/jig)
    echo "Headless binary size: ${SIZE} bytes"
    [ "$SIZE" -lt 5242880 ] || (echo "FAIL: headless binary exceeds 5MB ($SIZE bytes)" && exit 1)

release:
    cargo build --release --workspace
