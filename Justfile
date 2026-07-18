
default:
    @just --list

fmt:
    cargo +nightly fmt

lint:
    cargo clippy --workspace --all-targets

lint-fix:
    cargo clippy --workspace --all-targets --fix

test:
    cargo test --workspace

run TARGET:
    cargo run -p {{TARGET}}
