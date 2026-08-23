#!/usr/bin/env bash
set -euo pipefail

CONTRACT_NAME="${1:?Usage: deploy.sh <contract-name> <network>}"
NETWORK="${2:-testnet}"
WASM="target/wasm32v1-none/release/${CONTRACT_NAME//-/_}.wasm"

# Build the contract first so a stale or missing WASM is never deployed.
# deploy.sh always deploys the artifact that matches the current source.
cargo build --target wasm32v1-none --release -p "$CONTRACT_NAME"

if [ ! -f "$WASM" ]; then
  echo "Error: WASM file not found at $WASM after build"
  exit 1
fi
if [ ! -s "$WASM" ]; then
  echo "Error: WASM file at $WASM is empty"
  exit 1
fi

echo "Deploying $CONTRACT_NAME to $NETWORK..."
soroban contract deploy \
  --wasm "$WASM" \
  --network "$NETWORK" \
  --source "$SOROBAN_SECRET_KEY"
