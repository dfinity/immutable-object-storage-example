#!/usr/bin/env bash
# setup.sh — Automates Steps 1–6 of the Caffeine Object Storage getting-started guide.
#
# What this script does:
#   1. Checks for required tools (dfx, cargo)
#   2. Installs (or updates) the icfs CLI
#   3. Validates required environment variables
#   4. Funds your payment account with cycles
#   5. Deploys the chosen backend canister
#   6. Registers the storage gateway principal on the canister
#   7. Links the canister to your payment account
#
# Usage:
#   source .env          # or export variables manually — see README.md Step 2
#   ./scripts/setup.sh [--backend rust|motoko] [--network ic|local]
#
# Environment variables (all required):
#   CASHIER_CANISTER_ID   — Cashier canister ID (prod: 72ch2-fiaaa-aaaar-qbsvq-cai)
#   STORAGE_GATEWAY_URL   — Gateway URL         (prod: https://blob.caffeine.ai)
#   PRIVATE_KEY_FILE      — Path to your DFX identity PEM file
#   WALLET_CANISTER_ID    — Your DFX cycles wallet canister ID
#
# Optional environment variables:
#   TOP_UP_AMOUNT         — Cycles to deposit (default: 10T)
#   DAILY_LIMIT           — Daily spending limit per canister (default: 5T)

set -euo pipefail

# ── Defaults ─────────────────────────────────────────────────────────────────

BACKEND="${BACKEND:-rust}"
NETWORK="${NETWORK:-ic}"
TOP_UP_AMOUNT="${TOP_UP_AMOUNT:-10T}"
DAILY_LIMIT="${DAILY_LIMIT:-5T}"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
EXAMPLE_DIR="$(dirname "$SCRIPT_DIR")"

# ── Parse arguments ───────────────────────────────────────────────────────────

while [[ $# -gt 0 ]]; do
    case "$1" in
        --backend)
            BACKEND="$2"
            shift 2
            ;;
        --network)
            NETWORK="$2"
            shift 2
            ;;
        --top-up)
            TOP_UP_AMOUNT="$2"
            shift 2
            ;;
        --limit)
            DAILY_LIMIT="$2"
            shift 2
            ;;
        -h|--help)
            head -30 "$0" | grep '^#' | sed 's/^# \?//'
            exit 0
            ;;
        *)
            echo "Unknown argument: $1"
            exit 1
            ;;
    esac
done

# ── Helpers ───────────────────────────────────────────────────────────────────

info()    { echo "  [info]  $*"; }
success() { echo "  [ok]    $*"; }
error()   { echo "  [error] $*" >&2; exit 1; }
warn()    { echo "  [warn]  $*"; }

require_cmd() {
    command -v "$1" &>/dev/null || error "'$1' is not installed. See README.md Prerequisites."
}

# ── Step 1: Check prerequisites ───────────────────────────────────────────────

echo ""
echo "═══════════════════════════════════════════════"
echo "  Caffeine Object Storage — Setup"
echo "═══════════════════════════════════════════════"
echo ""

info "Checking prerequisites..."
require_cmd dfx
require_cmd cargo

DFX_VERSION=$(dfx --version | grep -oE '[0-9]+\.[0-9]+\.[0-9]+' | head -1)
info "dfx version: $DFX_VERSION"

CARGO_VERSION=$(cargo --version | grep -oE '[0-9]+\.[0-9]+\.[0-9]+' | head -1)
info "cargo version: $CARGO_VERSION"
success "Prerequisites OK"

# ── Step 2: Install icfs ──────────────────────────────────────────────────────

echo ""
info "Installing icfs CLI..."

curl -L https://caffeinelabs.github.io/object-storage/artifacts/icfs/latest/icfs-linux-x86_64 -o $HOME/.cargo/bin/icfs

success "icfs installed: $(icfs --version 2>/dev/null | head -1)"

# ── Step 3: Validate environment variables ────────────────────────────────────

echo ""
info "Validating environment variables..."

missing=()
[[ -z "${CASHIER_CANISTER_ID:-}" ]] && missing+=("CASHIER_CANISTER_ID")
[[ -z "${STORAGE_GATEWAY_URL:-}" ]] && missing+=("STORAGE_GATEWAY_URL")
[[ -z "${PRIVATE_KEY_FILE:-}" ]]    && missing+=("PRIVATE_KEY_FILE")
[[ -z "${WALLET_CANISTER_ID:-}" ]]  && missing+=("WALLET_CANISTER_ID")

if [[ ${#missing[@]} -gt 0 ]]; then
    error "Missing required environment variables: ${missing[*]}

    Set them in your shell or create a .env file and run: source .env
    See README.md Step 2 for details."
fi

[[ -f "$PRIVATE_KEY_FILE" ]] || error "PRIVATE_KEY_FILE does not exist: $PRIVATE_KEY_FILE"

info "CASHIER_CANISTER_ID = $CASHIER_CANISTER_ID"
info "STORAGE_GATEWAY_URL = $STORAGE_GATEWAY_URL"
info "PRIVATE_KEY_FILE    = $PRIVATE_KEY_FILE"
info "WALLET_CANISTER_ID  = $WALLET_CANISTER_ID"
success "Environment OK"

# ── Step 4: Fund payment account ──────────────────────────────────────────────

echo ""
info "Funding payment account with $TOP_UP_AMOUNT cycles..."

icfs cashier payment-account top-up --amount "$TOP_UP_AMOUNT" \
    || error "Top-up failed. Ensure WALLET_CANISTER_ID has sufficient cycles."

BALANCE=$(icfs cashier payment-account balance 2>/dev/null || echo "(could not retrieve)")
success "Payment account funded. Balance: $BALANCE"

# ── Step 5: Deploy backend canister ──────────────────────────────────────────

echo ""
info "Deploying $BACKEND backend canister to network: $NETWORK..."

case "$BACKEND" in
    rust)
        BACKEND_DIR="$EXAMPLE_DIR/rust-backend"
        ;;
    motoko)
        BACKEND_DIR="$EXAMPLE_DIR/motoko-backend"
        ;;
    *)
        error "Unknown backend: $BACKEND. Use 'rust' or 'motoko'."
        ;;
esac

[[ -d "$BACKEND_DIR" ]] || error "Backend directory not found: $BACKEND_DIR"

(
    cd "$BACKEND_DIR"

    if [[ "$BACKEND" == "motoko" ]]; then
        require_cmd mops
        info "Installing Motoko dependencies (mops install)..."
        mops install || error "mops install failed. Is mops installed? (npm i -g ic-mops)"
    fi

    if [[ "$NETWORK" == "local" ]]; then
        dfx start --background --clean 2>/dev/null || true
        sleep 2
    fi

    dfx deploy --network "$NETWORK" \
        || error "Canister deploy failed. Check dfx output above."
)

CANISTER_ID=$(cd "$BACKEND_DIR" && dfx canister id example_backend --network "$NETWORK")
success "Canister deployed: $CANISTER_ID"

# ── Step 6: Register the storage gateway principal ────────────────────────────

echo ""
info "Registering the storage gateway principal on canister $CANISTER_ID..."

GATEWAY_PRINCIPALS=$(
    dfx canister call "$CASHIER_CANISTER_ID" storage_gateway_list_v1 '()' --network "$NETWORK" 2>/dev/null
) || error "Failed to fetch gateway principal list from Cashier."

GATEWAY_PRINCIPAL=$(echo "$GATEWAY_PRINCIPALS" | grep -oP '[a-z0-9-]+' | head -1)
if [[ -z "$GATEWAY_PRINCIPAL" ]]; then
    error "Could not extract gateway principal from Cashier response."
fi

(
    cd "$BACKEND_DIR"
    dfx canister call example_backend add_gateway_principal \
        "(principal \"$GATEWAY_PRINCIPAL\")" --network "$NETWORK" \
        || error "Failed to register gateway principal."
)

success "Gateway principal registered: $GATEWAY_PRINCIPAL"

# ── Step 7: Link canister to payment account ──────────────────────────────────

echo ""
info "Linking canister $CANISTER_ID to payment account (limit: $DAILY_LIMIT/day)..."

icfs cashier payment-account add-canister \
    --paid-canister "$CANISTER_ID" \
    --limit "$DAILY_LIMIT" \
    || error "Failed to link canister. It may already be linked (run 'icfs cashier payment-account list-canisters' to check)."

success "Canister linked to payment account"

# ── Done ──────────────────────────────────────────────────────────────────────

echo ""
echo "═══════════════════════════════════════════════"
echo "  Setup complete!"
echo "═══════════════════════════════════════════════"
echo ""
echo "  Canister ID:  $CANISTER_ID"
echo "  Network:      $NETWORK"
echo "  Gateway:      $STORAGE_GATEWAY_URL"
echo ""
echo "  Upload a file:"
echo "    icfs blob upload --input-file <FILE> --owner $CANISTER_ID"
echo ""
echo "  (Optional) Deploy the frontend:"
echo "    cd $EXAMPLE_DIR/frontend && npm install && dfx deploy --network $NETWORK"
echo ""
