.PHONY: build run fmt clippy

build:
	cargo build

run:
	RUST_LOG=info cargo run -p appgate-auth -- --uds /run/appgate/pdp.sock --policy config/policy/foundry.toml & \
	RUST_LOG=info cargo run -p appgate-mod-http -- --bind 0.0.0.0:8080 --pdp-uds /run/appgate/pdp.sock --upstream http://127.0.0.1:3000

fmt:
	cargo fmt

clippy:
	cargo clippy --all-targets -- -D warnings