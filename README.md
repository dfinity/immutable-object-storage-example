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

---

## Architecture

There are four components in the system. As an integrator, you deploy and manage the
**Backend Canister**. The other three are operated by Caffeine.

```
                  ┌───────────────────────────────────────────────┐
                  │            Internet Computer                  │
                  │                                               │
                  │  ┌──────────────────┐  ┌──────────────────┐   │
 User/Browser ───►│  │  Your Backend    │  │ Cashier Canister  │  │
       │          │  │  Canister        │  │ (billing / auth)  │  │
       │          │  └──────────────────┘  └──────────────────┘   │
       │          └───────────────────────────────────────────────┘
       │                                          ▲
       │ PUT blob-tree + chunks                   │ budget check
       │ GET blob                                 │
       ▼                                          │
  ┌──────────────────────────────────┐            │
  │  Storage Gateway                 │────────────┘
  │  blob.caffeine.ai                │
  │  (verifies tree, cert & budget)  │──────────┐
  └──────────────────────────────────┘          │
       ▲                                        │ stores data
       │ periodic: BlobsAreLive checks,          ▼
       │ deletion confirmation           Object Storage
       │                                 (S3-compatible)
  ┌──────────────────────────┐
  │  Background Scrubber     │
  │  (garbage collection)    │
  └──────────────────────────┘
```

### Component roles

| Component | Operated by | Purpose |
|-----------|-------------|---------|
| **Your Backend Canister** | You | Stores blob hashes on-chain. Issues upload certificates. Tracks which blobs are live vs. deleted. |
| **Cashier Canister** | Caffeine | Manages payment accounts (cycles-based). Publishes the list of authorized gateway principals. Tracks budgets per data owner. |
| **Storage Gateway** | Caffeine | Accepts file uploads (after verifying the upload certificate and budget). Serves file downloads. Endpoint: `https://blob.caffeine.ai` |
| **Background Scrubber** | Caffeine | May periodically query your canister via `_immutableObjectStorageBlobsAreLive` to verify blobs are still needed. Calls `_immutableObjectStorageBlobsToDelete` and `_immutableObjectStorageConfirmBlobDeletion` to clean up deleted blobs. |

---

## Integration Checklist

This is the complete list of steps to integrate Caffeine Object Storage into your app.

### 1. Implement the storage protocol on your canister

Your backend canister must implement five `_immutableObjectStorage*` methods that the gateway
and scrubber call automatically.

See [Canister API Reference](#canister-api-reference) for the full Candid interface and
[the example backends](#repo-structure) for reference implementations in Rust and Motoko.

### 2. Install the `icfs` CLI

```bash
curl -L https://caffeinelabs.github.io/object-storage/artifacts/icfs/latest/icfs-linux-x86_64 -o icfs
chmod +x ./icfs
# Move to a directory in your PATH
```

### 3. Configure environment

```bash
export CASHIER_CANISTER_ID=72ch2-fiaaa-aaaar-qbsvq-cai
export STORAGE_GATEWAY_URL=https://blob.caffeine.ai
export NETWORK_URL=https://icp-api.io
export PRIVATE_KEY_FILE=~/.config/dfx/identity/default/identity.pem
export WALLET_CANISTER_ID=$(dfx identity get-wallet --network ic)
```

### 4. Fund your payment account

```bash
icfs cashier payment-account top-up --amount 10T
icfs cashier payment-account balance
```

### 5. Deploy your backend canister

```bash
# Rust
cd rust-backend && dfx deploy --network ic

# Motoko
cd motoko-backend && mops install && dfx deploy --network ic
```

You can optionally pass a custom Cashier canister ID at init time (see [Init Arguments](#init-arguments)).

### 6. Fetch gateway principals

The gateway needs to be authorized on your canister so it can manage blob lifecycle
(liveness checks, deletion confirmation). Call `_immutableObjectStorageUpdateGatewayPrincipals`
on your canister — it queries the Cashier for the current list of gateway principals and
stores them locally:

```bash
dfx canister call example_backend _immutableObjectStorageUpdateGatewayPrincipals '()' --network ic
```

**Why:** The gateway calls `_immutableObjectStorageBlobsAreLive`, `_immutableObjectStorageBlobsToDelete`,
and `_immutableObjectStorageConfirmBlobDeletion` on your canister. These methods check
`caller_is_gateway()` — the gateway must be in your authorized list.

**Note:** The list of gateways is dynamic and may change over time as new gateways are added
or removed. Call `_immutableObjectStorageUpdateGatewayPrincipals` periodically (or after
being notified of a gateway change) to stay up to date.

### 7. Link your canister to your payment account

```bash
icfs cashier payment-account add-canister \
  --paid-canister $(dfx canister id example_backend --network ic) \
  --limit 5T
```

**Why:** The Cashier needs to know which payment account covers storage costs for your canister.
The `--limit` controls the maximum daily spend.

### 8. Upload and download files

See the [Upload Protocol](#upload-protocol) section for the full TypeScript implementation,
or use the CLI:

```bash
# Upload
icfs blob upload --input-file ./my-photo.jpg \
  --owner $(dfx canister id example_backend --network ic)

# Download
icfs blob download \
  --owner $(dfx canister id example_backend --network ic) \
  --root-hash sha256:ba7816bf… \
  --output-file ./downloaded.jpg
```

---

## Upload Protocol

Uploading a file involves four steps: chunking + hashing, getting a certificate, sending the
blob tree, and sending each chunk. The frontend example implements this in full — see
[`frontend/src/storage-client.ts`](frontend/src/storage-client.ts).

### Step 1: Chunk the file and build a merkle tree

Files are split into 1 MiB (1,048,576 byte) chunks. Each chunk is hashed with the
domain separator `icfs-chunk/`:

```
chunk_hash = SHA-256("icfs-chunk/" || chunk_bytes)
```

The chunk hashes form the leaves of a binary merkle tree (type: **DSBMTWH** —
Domain-Separated Binary Merkle Tree With Headers). Internal nodes are computed with
the domain separator `ynode/`:

```
node_hash = SHA-256("ynode/" || left_child_hash || right_child_hash)
```

If a level has an odd number of nodes, the missing right sibling uses the sentinel
value `"UNBALANCED"` (the literal UTF-8 bytes, not a hash).

File metadata headers (Content-Type, Content-Length) are hashed with the domain
separator `icfs-metadata/` and combined with the chunk tree root:

```
metadata_hash = SHA-256("icfs-metadata/" || sorted_header_lines)
root_hash = SHA-256("ynode/" || chunks_root || metadata_hash)
```

The resulting root hash is formatted as `sha256:<64-hex-chars>`.

### Step 2: Get an upload certificate from your canister

Call `_immutableObjectStorageCreateCertificate(root_hash)` on your backend canister as
an **update call**. This does two things:

1. Records the hash as a live blob on your canister.
2. Returns `{ method: "upload", blob_hash: root_hash }`.

The important part is **not** the return value — it's the **IC response certificate**
attached to the update call response. This certificate proves that the canister authorized
the upload. Extract it from the response body:

```typescript
const result = await agent.call(canisterId, {
  methodName: '_immutableObjectStorageCreateCertificate',
  arg: IDL.encode([IDL.Text], [rootHash]),
});
if (isV4ResponseBody(result.response.body)) {
  const certificateBytes = result.response.body.certificate;
  // Use certificateBytes in the next step
}
```

### Step 3: Send the blob tree to the gateway

```
PUT {gateway}/v1/blob-tree/
Content-Type: application/json
```

Request body:
```json
{
  "blob_tree": {
    "tree_type": "DSBMTWH",
    "chunk_hashes": ["sha256:...", "sha256:..."],
    "tree": { "hash": "sha256:...", "left": {...}, "right": {...} },
    "headers": ["Content-Length: 12345", "Content-Type: application/octet-stream"]
  },
  "bucket_name": "default-bucket",
  "num_blob_bytes": 12345,
  "owner": "<your-canister-id>",
  "project_id": "0000000-0000-0000-0000-00000000000",
  "headers": ["Content-Length: 12345", "Content-Type: application/octet-stream"],
  "auth": {
    "OwnerEgressSignature": [/* certificate bytes as number array */]
  }
}
```

The gateway verifies that:
- The certificate is a valid IC response certificate
- The certified response contains `method: "upload"` and the matching `blob_hash`
- The canister has sufficient budget (checked against the Cashier)

Response:
```json
{
  "status": "blob_tree_accepted",
  "existing_chunks": ["sha256:aabb...", "sha256:ccdd..."],
  "chunk_check_errors": 0
}
```

The `existing_chunks` array lists chunk hashes that the gateway already has stored.
Skip uploading any chunks whose hashes appear in this list. If `chunk_check_errors`
is non-zero, some checks failed and those chunks should be re-uploaded (safe default).
If the response cannot be parsed (e.g. older gateway), fall back to uploading all chunks.

### Step 4: Upload each chunk

```
PUT {gateway}/v1/chunk/?owner_id=...&blob_hash=...&chunk_hash=...&chunk_index=...&bucket_name=...&project_id=...
Content-Type: application/octet-stream
Body: <raw chunk bytes>
```

Chunks can be uploaded in parallel (the example uses up to 10 concurrent uploads).
If a chunk already exists, the gateway returns `{ "status": "chunk_already_exists" }`
without re-storing it.

### Downloading a file

Construct the download URL:

```
GET {gateway}/v1/blob/?blob_hash=sha256:...&owner_id=<canister-id>&project_id=<project-id>
```

The gateway serves verified data — no client-side merkle proof verification is needed.

---

## Canister API Reference

Both the Rust and Motoko backends expose the same interface. The methods are organized into
three groups based on who calls them.

### Init Arguments

The canister optionally accepts a Cashier canister ID at init time. If not provided,
it defaults to the production Cashier (`72ch2-fiaaa-aaaar-qbsvq-cai`):

```candid
type InitArgs = record {
    cashier_canister_id : opt principal;
};

service : (opt InitArgs) -> { ... };
```

Pass a custom Cashier ID when deploying to a dev or test environment.

### User-facing API (called by your frontend)

| Method | Signature | Purpose |
|--------|-----------|---------|
| `_immutableObjectStorageCreateCertificate` | `(text) -> (CreateCertificateResult)` | Call with the `sha256:...` root hash before uploading. Records the blob as live and returns a certificate (via the IC response) that the gateway requires. |
| `set_blob_info` | `(text, text, nat64, text) -> ()` | Attach display metadata (name, size, content type) to a blob after upload. |
| `list_blobs` | `() -> (vec BlobInfo) query` | List all live blobs with metadata. |
| `delete_blob` | `(text) -> ()` | Mark a blob for deletion. The scrubber will remove it from storage. |

### Gateway / Scrubber API (called automatically — do not call from your frontend)

| Method | Signature | Called by | Purpose |
|--------|-----------|-----------|---------|
| `_immutableObjectStorageUpdateGatewayPrincipals` | `() -> ()` | Gateway / Admin | Queries the Cashier canister for the current list of gateway principals and stores them locally. Call after deployment and periodically to pick up gateway changes. |
| `_immutableObjectStorageBlobsAreLive` | `(vec blob) -> (vec bool) query` | Background Scrubber | May periodically check whether blobs (each identified by a 32-byte hash) are still needed. Returns a `vec bool` in the same order as the input — `true` if the blob is live and not marked for deletion. This  |
| `_immutableObjectStorageBlobsToDelete` | `() -> (vec text) query` | Background Scrubber | Returns hashes of blobs marked for deletion. Only responds to authorized gateway principals. |
| `_immutableObjectStorageConfirmBlobDeletion` | `(vec blob) -> ()` | Background Scrubber | Confirms blobs have been removed from storage. The canister then removes them from its state. |

### Candid types

```candid
type BlobInfo = record {
    hash         : text;
    name         : text;
    size         : nat64;
    content_type : text;
    created_at   : nat64;
};

type CreateCertificateResult = record { method : text; blob_hash : text };

type InitArgs = record {
    cashier_canister_id : opt principal;
};
```

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

Install the prerequisites below, then run the [recommended test script](#run-tests-recommended) to
build and verify the example. When you are ready to use it on the IC, follow the
[Integration Checklist](#integration-checklist).

### Prerequisites — install these first

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

For **production deployment** you also need a **cycles wallet** with enough cycles to fund the
payment account (at least 10 T cycles recommended, more is better). Obtain cycles by converting ICP via the
[NNS dapp](https://nns.ic0.app).

### Run tests (recommended)

From the repo root:

```bash
python3 scripts/run_tests.py
```

The script checks for `cargo`, `dfx`, and `mops`; downloads the PocketIC binary into `.tools/` if
missing; builds both backends; and runs Rust unit tests and PocketIC canister tests.

Manual alternative: see [docs/automation-options.md](docs/automation-options.md).

### (Optional) Deploy the frontend

```bash
cd frontend
npm install
dfx deploy --network ic
```

Configure the frontend via `.env`:

| Variable | Default | Purpose |
|----------|---------|---------|
| `VITE_CANISTER_ID` | — | Your deployed backend canister ID (required) |
| `VITE_STORAGE_GATEWAY_URL` | `https://blob.caffeine.ai` | Storage gateway URL |
| `VITE_IC_URL` | `https://icp-api.io` | IC network URL |

---

## Monitoring

```bash
# Current balance
icfs cashier payment-account balance

# Full audit log
icfs cashier payment-account audit-log

# Canister-specific spending
icfs cashier payment-account audit-log | grep <CANISTER_ID>
```

Prometheus metrics exposed by the Cashier:

| Metric                                                            | Alert when…           |
|-------------------------------------------------------------------|-----------------------|
| `ic_cashier_payment_account_balance{owner="<principal>"}`         | `< 5T`                |
| `ic_cashier_auto_topup_skipped_insufficient_balance_total`        | increases             |

---

## Repo Structure

```
immutable-object-storage-example/
├── README.md               This file
├── scripts/
│   ├── run_tests.py        Build + run all tests (Python 3.10+)
│   └── setup.sh            Automates deploy steps
├── rust-backend/           Rust ic-cdk canister
│   ├── Cargo.toml
│   ├── dfx.json
│   └── src/
│       ├── lib.rs          Canister entry + app API
│       └── storage.rs      Storage protocol implementation
├── motoko-backend/         Motoko canister
│   ├── dfx.json
│   ├── mops.toml
│   └── src/main.mo
├── tests/                  PocketIC canister tests
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs          Shared helpers and Candid types
│       ├── rust_backend.rs
│       └── motoko_backend.rs
└── frontend/               React + Vite frontend
    ├── package.json
    ├── dfx.json
    └── src/
        ├── App.tsx          Upload/download UI
        ├── canister.ts      IC agent wiring
        └── storage-client.ts  Full upload protocol implementation
```

## Running the Automated Setup

If you prefer a single script over the manual steps:

```bash
./scripts/setup.sh
```

The script installs `icfs`, validates environment variables, tops up your payment account,
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
