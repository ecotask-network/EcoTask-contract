.PHONY: all build test bench fmt lint clean deploy-testnet

all: build test lint fmt

build:
	cargo build --target wasm32v1-none --release

test:
	cargo test --workspace

# Run only the Soroban budget / footprint benchmark tests and show their output.
# Limits are documented in tests/reward_integration_test.rs.
bench:
	cargo test --workspace budget -- --nocapture

fmt:
	cargo fmt --all -- --check

lint:
	cargo clippy --all-targets --all-features -- -D warnings

clean:
	cargo clean

deploy-testnet: build
	@echo "Deploying contracts to testnet..."
	./scripts/deploy.sh eco-token testnet
	./scripts/deploy.sh task-registry testnet
	./scripts/deploy.sh reward-engine testnet
