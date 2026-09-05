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
