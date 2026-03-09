# Caffeine Object Storage — Example App

This example shows how to integrate [Caffeine Immutable Object Storage](https://blob.caffeine.ai)
into an Internet Computer app. It includes a Rust backend canister, a Motoko backend canister, and
a React frontend. Pick the backend language that suits your project.

## What is Caffeine Object Storage?

Caffeine Object Storage lets ICP canisters store large immutable files (images, videos, documents,
models — up to 5 TB per file) off-chain while keeping cryptographic references on-chain. Your
canister stores only a 32-byte SHA-256 hash per file. The storage gateway handles the actual bytes,
verifying every upload against that hash.

**Key properties:**

- Content-addressed: the hash *is* the address. If the bytes change, the hash changes.
- Payment in ICP cycles via the Cycles Ledger. No separate accounts or tokens required.
- No vendor lock-in on the data format: SHA-256 hashes are a universal standard.

### Architecture

```
                  ┌──────────────────────────────────────────┐
                  │          Internet Computer               │
                  │                                          │
                  │  ┌──────────────┐  ┌──────────────────┐  │
 User/Browser ───►│  │ Your Backend │  │ Cashier Canister │  │
       │          │  │   Canister   │  │  (billing/auth)  │  │
       │          │  └──────────────┘  └──────────────────┘  │
       │          └──────────────────────────────────────────┘
       │
       │ PUT/GET blob
       ▼
  ┌─────────────────────────────────┐
  │  Storage Gateway            │
  │  blob.caffeine.ai           │
  │  (verifies hash & balance)  │
  └────────────────┬────────────┘
                   │ stores data
                   ▼
              Object Storage
              (S3-compatible)
```

### How a file upload works

1. The frontend computes a SHA-256 hash of the file.
2. The frontend calls `_immutableObjectStorageCreateCertificate(hash)` on your canister. Your
   canister records that this hash is expected and returns a signed certificate.
3. The frontend `PUT`s the file bytes to `https://blob.caffeine.ai/blob/<canister-id>/<hash>`,
   attaching the certificate as a header.
4. The gateway checks your canister's balance (via the Cashier), verifies the certificate, and
   accepts the upload only if both pass.
5. The file is now accessible at the same URL via `GET`.

---

## Pricing

> **Disclaimer:** Pricing is set by Caffeine and may change at any time as upstream ICP resource
> costs (compute, storage, network) change. Always query the live price list before making
> cost-sensitive decisions.

Query the current price list at any time:

```bash
dfx canister call 72ch2-fiaaa-aaaar-qbsvq-cai pricelist_v1 '()' --network ic
```

Current charges include:

| Resource         | Notes                                              |
|------------------|----------------------------------------------------|
| Storage          | Charged per GB per 30 days (prepaid)               |
| Upload           | Charged per GB uploaded                            |
| Download         | Charged per GB downloaded                          |
| Requests         | Charged per 1 000 read/write requests              |

Storage is prepaid for 30 days on upload. If your balance reaches zero, existing data is retained
but inaccessible until you top up. After 30 days of zero balance, data is deleted.

---

## Current Status

**Single Caffeine-maintained gateway.** All uploads and downloads are served by a single gateway
operated by Caffeine at `https://blob.caffeine.ai`. This provides a simple, reliable starting
point.

**Near future — multiple gateways.** We plan to add support for multiple independent gateways so
you can choose between them based on latency, pricing, or trust. If you need multi-gateway support
sooner, [submit a feature request](https://github.com/caffeinelabs/object-storage/issues/new?template=feature_request.md&title=Multi-gateway+support).

**Payment — Cycles Ledger.** The gateway currently accepts payment in ICP cycles via the
[Cycles Ledger](https://internetcomputer.org/docs/current/developer-docs/defi/cycles/cycles-ledger).
Support for additional ledgers (e.g. ckUSDC, ckUSDT) is planned. If you need a specific ledger
sooner, [submit a feature request](https://github.com/caffeinelabs/object-storage/issues/new?template=feature_request.md&title=Payment+ledger+support).

---

## Getting Started

Install the prerequisites below, then run the [recommended test script](#run-tests-recommended) to build and verify the example. When you are ready to use it on the IC, follow the deploy steps (icfs, environment, fund account, deploy canister, register gateway, link payment account).

### Prerequisites — install these first

Install the tools below, then verify each one. You need **Rust** and **dfx** for both backends; **mops** and **Node** only if you use the Motoko backend or the frontend.

| Tool    | Version   | Install |
|---------|-----------|---------|
| **Rust** (cargo) | stable | `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \| sh` |
| **dfx** | ≥ 0.28 | `sh -ci "$(curl -fsSL https://internetcomputer.org/install.sh)"` |
| **mops** | latest | `curl -fsSL cli.mops.one/install.sh \| sh` (or `npm i -g ic-mops`) — only for Motoko backend |
| **Node** | ≥ 20 | [nodejs.org](https://nodejs.org) — for frontend and npm-based mops |
| **Python** | 3.10+ | For the [recommended test script](#run-tests-recommended); PocketIC is downloaded by the script when needed. |

Verify:

```bash
cargo --version
dfx --version
# If using Motoko: mops --version
# If using frontend: node --version
# For the test script: python3 --version  (3.10+)
```

For **production deployment** you also need a **cycles wallet** with enough cycles to fund the payment account (10 T cycles recommended). Obtain cycles by converting ICP via the [NNS dapp](https://nns.ic0.app).

### Run tests (recommended)

From the `immutable-object-storage-example` directory, run:

```bash
python3 scripts/run_tests.py
```

The script (Python 3.10+) checks for `cargo`, `dfx`, and `mops`; downloads the PocketIC binary into `.tools/` if missing; builds both backends; and runs Rust unit tests and PocketIC canister tests. You must have the prerequisites above installed first.

Manual alternative (not recommended): see [docs/automation-options.md](docs/automation-options.md) for manual build/test steps and other automation (mise, Docker).

### Step 1 — Install `icfs`

`icfs` is the Caffeine CLI for managing your storage account and uploading files.

```bash
cargo install --git https://github.com/caffeinelabs/object-storage icfs
```

Verify:

```bash
icfs --version
```

### Step 2 — Configure environment

```bash
# Production endpoints (use these unless you are a Caffeine developer)
export CASHIER_CANISTER_ID=72ch2-fiaaa-aaaar-qbsvq-cai
export STORAGE_GATEWAY_URL=https://blob.caffeine.ai
export NETWORK_URL=https://icp-api.io

# Your DFX identity key (the owner of your payment account)
export PRIVATE_KEY_FILE=~/.config/dfx/identity/default/identity.pem

# Your dfx cycles wallet (needed for top-ups; cycles can only be sent from canisters)
export WALLET_CANISTER_ID=$(dfx identity get-wallet --network ic)
```

> Tip: Add these to a `.env` file and `source` it at the start of each session.

### Step 3 — Fund your payment account

Your payment account is identified by your identity principal. Top it up with cycles:

```bash
icfs cashier payment-account top-up --amount 10T
```

Check the balance:

```bash
icfs cashier payment-account balance
```

### Step 4 — Deploy your backend canister

Choose either the Rust or Motoko backend. Both implement the same interface.

**Rust backend:**

```bash
cd rust-backend
dfx deploy --network ic
```

**Motoko backend:**

```bash
cd motoko-backend
mops install
dfx deploy --network ic
```

Note your canister ID from the deploy output, or retrieve it later:

```bash
dfx canister id example_backend --network ic
```

### Step 5 — Register the storage gateway

After deploying, register the storage gateway principal so it can manage blob
lifecycle on your canister. The gateway principal is published by the Cashier:

```bash
# Fetch the gateway principal list from the Cashier
GATEWAY_PRINCIPAL=$(dfx canister call 72ch2-fiaaa-aaaar-qbsvq-cai storage_gateway_list_v1 '()' --network ic \
  | grep -oP '[a-z0-9-]+' | head -1)

# Register it on your canister (only canister controllers can call this)
dfx canister call example_backend add_gateway_principal "(principal \"$GATEWAY_PRINCIPAL\")" --network ic
```

### Step 6 — Link your canister to your payment account

```bash
icfs cashier payment-account add-canister \
  --paid-canister $(dfx canister id example_backend --network ic) \
  --limit 5T
```

This tells the Cashier that your payment account will cover up to 5 T cycles per day for this
canister. Adjust `--limit` based on your expected usage.

Verify:

```bash
icfs cashier payment-account list-canisters
```

### Step 7 — Upload your first file

```bash
icfs blob upload \
  --input-file ./my-photo.jpg \
  --owner $(dfx canister id example_backend --network ic)
```

The CLI prints the blob hash (e.g. `sha256:ba7816bf…`). Download it back:

```bash
icfs blob download \
  --owner $(dfx canister id example_backend --network ic) \
  --root-hash sha256:ba7816bf… \
  --output-file ./downloaded.jpg
```

### Step 8 — (Optional) Deploy the frontend

The frontend is a React app that lets users upload, browse, and download files through a browser.

```bash
cd frontend
npm install
dfx deploy --network ic
```

Open the URL printed by `dfx deploy` in your browser.

### Next steps: what to change for your app

After deploying, point the frontend (or your own client) at your backend and adjust config as needed:

| What | Where | What to set |
|------|--------|--------------|
| **Backend canister ID** | Frontend: create `frontend/.env` (or `.env.local`) | `VITE_CANISTER_ID=<your-example_backend-canister-id>` so the UI talks to your deployed backend. |
| **Frontend dfx remote** | `frontend/dfx.json` → `example_backend.remote.id.ic` | Replace `REPLACE_WITH_YOUR_CANISTER_ID` with your backend canister ID if you deploy the frontend with a pre-deployed backend. |
| **Gateway / network (optional)** | Frontend: `.env` | `VITE_STORAGE_GATEWAY_URL`, `VITE_IC_URL` for dev/staging (defaults: prod blob.caffeine.ai and icp-api.io). |

For your own canister (not this example), you would: implement the same [Candid interface](#canister-api-reference), register the storage gateway via `add_gateway_principal`, link the canister to your payment account with `icfs cashier payment-account add-canister`, and ensure callers use the certificate flow for uploads.

---

## Canister API Reference

Both the Rust and Motoko backends expose the same Candid interface. The gateway-facing methods
(`_immutableObjectStorage*`) are called by the storage gateway — do not call them directly from
your frontend.

```candid
type BlobInfo = record {
    hash         : text;
    name         : text;
    size         : nat64;
    content_type : text;
    created_at   : nat64;
};

type CreateCertificateResult = record { method : text; blob_hash : text };

service : () -> {
    // ── Called by the storage gateway (do not call directly) ──────────────────

    _immutableObjectStorageUpdateGatewayPrincipals : () -> ();
    _immutableObjectStorageBlobIsLive : (blob) -> (bool) query;
    _immutableObjectStorageBlobsToDelete : () -> (vec text) query;
    _immutableObjectStorageConfirmBlobDeletion : (vec blob) -> ();
    _immutableObjectStorageCreateCertificate : (text) -> (CreateCertificateResult);

    // ── Admin (canister controller only) ─────────────────────────────────────

    // Register a gateway principal so it can manage blob deletion.
    add_gateway_principal : (principal) -> ();

    // ── User-facing API ──────────────────────────────────────────────────────

    set_blob_info : (text, text, nat64, text) -> ();
    list_blobs : () -> (vec BlobInfo) query;
    delete_blob : (text) -> ();
};
```

---

## Monitoring

Check your payment account balance and spending:

```bash
# Current balance
icfs cashier payment-account balance

# Full audit log (all transactions)
icfs cashier payment-account audit-log

# Canister-specific spending
icfs cashier payment-account audit-log | grep <CANISTER_ID>
```

Set up alerts on these Prometheus metrics exposed by the Cashier:

| Metric                                                      | Alert when…           |
|-------------------------------------------------------------|-----------------------|
| `account_balance{account="<principal>"}`                    | `< 5T`                |
| `auto_topup_skipped_total{reason="insufficient_payment_account"}` | increases       |

---

## Repo Structure

```
example/
├── README.md              This file
├── scripts/
│   ├── run_tests.py       Recommended: build + run all tests (Python 3.10+)
│   └── setup.sh           Automates deploy steps (icfs, gateway, payment account)
├── rust-backend/          Rust ic-cdk canister
│   ├── Cargo.toml
│   ├── dfx.json
│   └── src/
│       ├── lib.rs         Canister entry point + app API
│       └── storage.rs     Storage protocol implementation
├── motoko-backend/        Motoko canister
│   ├── dfx.json
│   ├── mops.toml
│   └── src/main.mo
├── tests/                 PocketIC canister tests (Rust + Motoko)
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs         Shared helpers and Candid types
│       ├── rust_backend.rs
│       └── motoko_backend.rs
└── frontend/              React + Vite frontend
    ├── package.json
    ├── dfx.json
    └── src/
```

## Testing

**Recommended:** Use the [Run tests (recommended)](#run-tests-recommended) script above. It builds both backends, ensures PocketIC is available (downloads it if needed), and runs Rust unit tests plus PocketIC canister tests for both backends.

What the script does: builds `rust-backend` and `motoko-backend` WASMs, runs `cargo test` in `rust-backend` (unit tests for storage logic), then runs `cargo test` in `tests/` (PocketIC tests against both WASMs). Manual steps and other options (mise, Docker) are in [docs/automation-options.md](docs/automation-options.md).

## Running the Automated Setup

If you prefer a single script over the manual steps, run:

```bash
cd example
./scripts/setup.sh
```

The script installs `icfs`, validates your environment variables, tops up your payment account,
deploys the canister, registers the gateway principal, and links it to your payment account.

---

## Links

| Resource                  | URL                                                                             |
|---------------------------|---------------------------------------------------------------------------------|
| Storage Gateway (prod)    | https://blob.caffeine.ai                                                        |
| Cashier canister (prod)   | https://dashboard.internetcomputer.org/canister/72ch2-fiaaa-aaaar-qbsvq-cai    |
| Storage Gateway (dev)     | https://dev-blob.caffeine.ai                                                    |
| Cashier canister (dev)    | https://dashboard.internetcomputer.org/canister/xc7sj-uyaaa-aaaaf-qbrja-cai   |
| Price list                | `dfx canister call 72ch2-fiaaa-aaaar-qbsvq-cai pricelist_v1 '()' --network ic` |
| Feature requests          | https://github.com/caffeinelabs/object-storage/issues                           |
