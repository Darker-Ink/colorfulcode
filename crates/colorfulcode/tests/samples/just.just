# Task runner for the project
set shell := ["bash", "-uc"]
set dotenv-load

export RUST_LOG := "info"
version := `git describe --tags`
target := if os() == "windows" { "x86_64-pc-windows-msvc" } else { "x86_64-unknown-linux-gnu" }

alias b := build

# List the recipes
default:
    @just --list

# Build in release mode
build profile='release' *flags: fmt
    cargo build --profile {{profile}} {{flags}}
    @echo "built {{version}} for {{target}}"

test filter="": (build "dev")
    cargo test {{filter}} -- --nocapture
    -rm -f target/*.profraw

[private]
fmt:
    cargo fmt --all

[group('release')]
publish $TOKEN: build test
    #!/usr/bin/env bash
    set -euo pipefail
    if [[ -z "$TOKEN" ]]; then echo "missing token"; exit 1; fi
    cargo publish --token "$TOKEN"

stats:
    #!/usr/bin/env python3
    import os
    print(len(os.listdir("src")), "files")
