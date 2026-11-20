#!/usr/bin/env bash
# The KYC demo on Stellar testnet, end to end.
#
# Deploys the verifier contract, registers the reproducible zkVM guest with it,
# proves one KYC risk score with Groth16 and records it on chain through
# verify_receipt. Every step prints the command it runs, and the stellar CLI
# prints a link to each transaction.
#
# Run it from the repository root:
#
#   examples/kyc-demo/deploy.sh [--opening FILE] [--bundle FILE] [--risc0-params FILE] [--contract-id ID]
#
#   --opening FILE       The model's opening, the secret that keeps its
#                        commitment hiding. Default target/kyc-demo/model.opening,
#                        generated from 32 random bytes if it does not exist.
#                        Keep it: proving again for this registration needs it.
#   --bundle FILE        Submit this bundle (from `prove --groth16`) instead of
#                        proving here.
#   --risc0-params FILE  Register these RISC Zero values (from
#                        `export-vk --format json`) instead of exporting them here.
#   --contract-id ID     Use a contract this identity already administers
#                        instead of deploying a new one.
#
# Proving and exporting build the guest with ZKML_GUEST_DOCKER=1, so they need
# x86_64 Linux with Docker; proving takes about 25 minutes of CPU. On another
# machine, make the two files on such a host and pass them with --bundle and
# --risc0-params: the rest only needs cargo, python3 and the stellar CLI.
#
# Environment:
#   ZKML_IDENTITY  stellar identity that deploys and administers the contract
#                  (default zkml-kyc-demo; generated and funded by friendbot
#                  when it does not exist yet)
#   ZKML_NETWORK   stellar network (default testnet)
#   KYC_MODEL      model to prove (default examples/models/kyc_tree.json)
#   KYC_INPUT      its features (default 0.6,0.4,0.0, the high-risk branch)

set -euo pipefail

IDENTITY="${ZKML_IDENTITY:-zkml-kyc-demo}"
NETWORK="${ZKML_NETWORK:-testnet}"
MODEL="${KYC_MODEL:-examples/models/kyc_tree.json}"
INPUT="${KYC_INPUT:-0.6,0.4,0.0}"
WASM="target/wasm32v1-none/contract/zkml_verifier.wasm"
WORK="target/kyc-demo"

BUNDLE=""
PARAMS=""
CONTRACT_ID=""
OPENING="$WORK/model.opening"
while [ $# -gt 0 ]; do
    case "$1" in
        --bundle) BUNDLE="$2"; shift 2 ;;
        --risc0-params) PARAMS="$2"; shift 2 ;;
        --contract-id) CONTRACT_ID="$2"; shift 2 ;;
        --opening) OPENING="$2"; shift 2 ;;
        -h|--help) sed -n '2,36p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
        *) echo "unknown argument: $1 (see --help)" >&2; exit 2 ;;
    esac
done

step() { printf '\n== %s\n' "$*"; }
run() { printf '$ %s\n' "$*"; "$@"; }
die() { echo "error: $*" >&2; exit 1; }

[ -f Cargo.toml ] && [ -d crates/zkml-verifier ] || die "run this from the repository root"
command -v stellar >/dev/null || die "the stellar CLI is not on PATH: https://developers.stellar.org/docs/tools/cli"
PYTHON="$(command -v python3 || command -v python || true)"
[ -n "$PYTHON" ] || die "python3 is needed to read the JSON files"
mkdir -p "$WORK"

# Read one top-level field of a JSON file, without a trailing carriage return.
json_field() {
    "$PYTHON" -c 'import json, sys; print(json.load(open(sys.argv[1]))[sys.argv[2]])' "$1" "$2" | tr -d '\r'
}

step "1. Identity ($IDENTITY on $NETWORK)"
if ! stellar keys address "$IDENTITY" >/dev/null 2>&1; then
    run stellar keys generate "$IDENTITY" --network "$NETWORK" --fund
fi
ADMIN="$(stellar keys address "$IDENTITY" | tr -d '\r')"
echo "admin: $ADMIN"

step "2. Model commitment"
if [ ! -f "$OPENING" ]; then
    # 32 bytes from the operating system's secure random source, readable only
    # by this user.
    (umask 077 && "$PYTHON" -c 'import secrets; print(secrets.token_hex(32))' > "$OPENING")
    echo "generated a new opening in $OPENING: keep it private, and keep it"
fi
MODEL_HASH="$(cargo run -q -p zkml-prover -- commit "$MODEL" --opening "$OPENING" | tr -d '\r')"
echo "$MODEL: $MODEL_HASH"

step "3. RISC Zero values for the reproducible guest"
if [ -z "$PARAMS" ]; then
    PARAMS="$WORK/risc0_params.json"
    run env ZKML_GUEST_DOCKER=1 cargo run -q -p zkml-prover --features zkvm -- \
        export-vk --format json -o "$PARAMS"
fi
IMAGE_ID="$(json_field "$PARAMS" image_id)"
echo "image id: $IMAGE_ID"

step "4. Bundle"
if [ -z "$BUNDLE" ]; then
    BUNDLE="$WORK/bundle.json"
    run env ZKML_GUEST_DOCKER=1 RISC0_DEV_MODE=0 cargo run -q -p zkml-prover --features groth16 -- \
        prove "$MODEL" -i "$INPUT" --opening "$OPENING" --groth16 -o "$BUNDLE"
fi
SEAL="$(json_field "$BUNDLE" seal)"
JOURNAL="$(json_field "$BUNDLE" journal)"
[ "$(json_field "$BUNDLE" image_id)" = "$IMAGE_ID" ] \
    || die "$BUNDLE was proved by guest $(json_field "$BUNDLE" image_id), not $IMAGE_ID; prove and export with the same ZKML_GUEST_DOCKER=1 build"
# Journal bytes 8..40 are the model hash: refuse here what the contract would.
[ "${JOURNAL:16:64}" = "$MODEL_HASH" ] \
    || die "$BUNDLE is for model commitment ${JOURNAL:16:64}, not $MODEL under $OPENING ($MODEL_HASH)"
echo "seal: ${#SEAL} hex chars, journal: ${#JOURNAL} hex chars"

if [ -z "$CONTRACT_ID" ]; then
    step "5. Build and deploy the verifier (constructor: admin, model hash, no native key)"
    run cargo build -q -p zkml-verifier --target wasm32v1-none --profile contract
    printf '$ stellar contract deploy --wasm %s --source-account %s --network %s -- --admin %s --model_hash %s\n' \
        "$WASM" "$IDENTITY" "$NETWORK" "$ADMIN" "$MODEL_HASH"
    CONTRACT_ID="$(stellar contract deploy --wasm "$WASM" --source-account "$IDENTITY" --network "$NETWORK" \
        -- --admin "$ADMIN" --model_hash "$MODEL_HASH" | tr -d '\r')"
fi
echo "contract: $CONTRACT_ID"

invoke() {
    run stellar contract invoke --id "$CONTRACT_ID" --source-account "$IDENTITY" --network "$NETWORK" -- "$@"
}

step "6. Register the guest and RISC Zero's verifying key"
CONFIG="$("$PYTHON" -c '
import json, sys
p = json.load(open(sys.argv[1]))
keys = ("image_id", "control_root", "bn254_control_id", "selector")
print(json.dumps({k: p[k] for k in keys}, separators=(",", ":")))' "$PARAMS" | tr -d '\r')"
VK="$("$PYTHON" -c '
import json, sys
print(json.dumps(json.load(open(sys.argv[1]))["vk"], separators=(",", ":")))' "$PARAMS" | tr -d '\r')"
invoke set_risc0_config --config "$CONFIG"
invoke set_risc0_vk --vk "$VK"

step "7. Verify the receipt on chain"
invoke verify_receipt --seal "$SEAL" --journal "$JOURNAL"

step "8. Read the recorded result"
invoke get_result
invoke get_verification_count

step "Done"
echo "contract: $CONTRACT_ID"
echo "explorer: https://stellar.expert/explorer/$NETWORK/contract/$CONTRACT_ID"
