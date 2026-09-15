set shell := ["bash", "-cu"]

default:
  @just --list

build:
  cargo build

install:
  cargo install --path . --locked

test:
  cargo test

lint:
  cargo clippy --all-targets -- -D warnings

fmt:
  cargo fmt

fmt-check:
  cargo fmt --check

run *args:
  cargo run -- {{args}}

# Full CI pipeline (same commands run in GitHub Actions)
ci: fmt-check lint test build-locked

# Verify the locked build (release.yml uses --locked; catches Cargo.lock drift)
build-locked:
  cargo build --locked
