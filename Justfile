
default:
    @just --list

fmt:
    cargo +nightly fmt

lint:
    cargo clippy

lint-fix:
    cargo clippy --fix
