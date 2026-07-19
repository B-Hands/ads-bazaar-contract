#!/usr/bin/env bash
# Builds, optimizes, deploys, and initializes both AdsBazaar contracts
# (campaign-escrow and dispute-resolution) on a Stellar network in one
# command. See README.md#deploy-to-testnet for prerequisites and usage.
set -euo pipefail

if [ -f .env ]; then
  # shellcheck disable=SC1091
  source .env
fi

: "${DEPLOYER_SECRET:?Set DEPLOYER_SECRET in .env (see .env.example)}"
: "${ADMIN_ADDRESS:?Set ADMIN_ADDRESS in .env (see .env.example)}"
: "${ADMIN_SECRET:?Set ADMIN_SECRET in .env (see .env.example)}"
: "${FEE_BPS:?Set FEE_BPS in .env (see .env.example)}"
: "${STELLAR_NETWORK:=testnet}"

WASM_DIR="target/wasm32v1-none/release"
ESCROW_WASM="ads_bazaar_campaign_escrow"
DISPUTE_WASM="ads_bazaar_dispute_resolution"
ENV_FILE=".env.${STELLAR_NETWORK}"

echo "==> Building release wasm for network: ${STELLAR_NETWORK}"
stellar contract build

echo "==> Optimizing ${ESCROW_WASM}.wasm"
stellar contract optimize --wasm "${WASM_DIR}/${ESCROW_WASM}.wasm"

echo "==> Optimizing ${DISPUTE_WASM}.wasm"
stellar contract optimize --wasm "${WASM_DIR}/${DISPUTE_WASM}.wasm"

echo "==> Deploying campaign-escrow"
ESCROW_ID=$(stellar contract deploy \
  --wasm "${WASM_DIR}/${ESCROW_WASM}.optimized.wasm" \
  --source "${DEPLOYER_SECRET}" \
  --network "${STELLAR_NETWORK}")
echo "campaign-escrow deployed: ${ESCROW_ID}"

echo "==> Deploying dispute-resolution"
DISPUTE_ID=$(stellar contract deploy \
  --wasm "${WASM_DIR}/${DISPUTE_WASM}.optimized.wasm" \
  --source "${DEPLOYER_SECRET}" \
  --network "${STELLAR_NETWORK}")
echo "dispute-resolution deployed: ${DISPUTE_ID}"

echo "==> Initializing campaign-escrow"
stellar contract invoke \
  --id "${ESCROW_ID}" \
  --source "${ADMIN_SECRET}" \
  --network "${STELLAR_NETWORK}" \
  -- initialize \
  --admin "${ADMIN_ADDRESS}" \
  --dispute_contract "${DISPUTE_ID}" \
  --fee_bps "${FEE_BPS}"

echo "==> Initializing dispute-resolution"
stellar contract invoke \
  --id "${DISPUTE_ID}" \
  --source "${ADMIN_SECRET}" \
  --network "${STELLAR_NETWORK}" \
  -- initialize \
  --admin "${ADMIN_ADDRESS}" \
  --escrow_contract "${ESCROW_ID}"

{
  echo "CAMPAIGN_ESCROW_ID=${ESCROW_ID}"
  echo "DISPUTE_RESOLUTION_ID=${DISPUTE_ID}"
} >> "${ENV_FILE}"

echo "==> Done. Contract IDs saved to ${ENV_FILE}"
echo "campaign-escrow:     ${ESCROW_ID}"
echo "dispute-resolution:  ${DISPUTE_ID}"
