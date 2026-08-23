#!/usr/bin/env bash
set -euo pipefail

if [[ -z "${1:-}" ]]; then
  echo "Usage: fund-accounts.sh <address1> [address2 ...]" >&2
  exit 1
fi

NETWORK="${NETWORK:-testnet}"
if [[ "$NETWORK" != "testnet" && "$NETWORK" != "futurenet" ]]; then
  echo "Error: NETWORK must be 'testnet' or 'futurenet'. Current NETWORK is '$NETWORK'." >&2
  exit 1
fi

for addr in "$@"; do
  if [[ ! "$addr" =~ ^G[A-Z2-7]{55}$ ]]; then
    echo "Error: Invalid address format '$addr'. Must start with G and be 56 characters long." >&2
    exit 1
  fi

  echo "Funding $addr on $NETWORK..."

  if [[ "$NETWORK" == "testnet" ]]; then
    endpoint="https://friendbot.stellar.org"
  else
    endpoint="https://friendbot.futurenet.stellar.org"
  fi

  if response=$(curl --fail -sS "$endpoint?addr=$addr" 2>&1); then
    if echo "$response" | grep -q '"successful": true' || echo "$response" | grep -q '"hash"'; then
      echo "Successfully funded $addr"
    else
      echo "Unexpected success response format for $addr:"
      echo "$response"
    fi
  else
    exit_code=$?
    echo "Error: Failed to fund $addr. (curl exit code: $exit_code)" >&2
    echo "$response" >&2
    exit 1
  fi
done

echo "Done."
