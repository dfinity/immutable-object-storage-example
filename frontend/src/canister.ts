/**
 * canister.ts — ic-agent wiring for the example backend canister.
 *
 * This module creates an actor that talks to the example backend canister
 * using the Candid interface defined in rust-backend/src/example_backend.did.
 *
 * For production deployments replace the placeholder values with your own
 * canister ID and network URL.
 */

import { Actor, HttpAgent } from "@dfinity/agent";
import { idlFactory } from "./declarations/example_backend.did";

// ── Configuration ─────────────────────────────────────────────────────────────

// The canister ID of the deployed backend.
// Set VITE_CANISTER_ID in your .env file (dfx writes it to .env automatically).
export const CANISTER_ID: string =
  import.meta.env.VITE_CANISTER_ID ??
  (import.meta.env.VITE_CANISTER_ID_EXAMPLE_BACKEND as string | undefined) ??
  "";

// The storage gateway base URL.
export const GATEWAY_URL: string =
  import.meta.env.VITE_STORAGE_GATEWAY_URL ?? "https://blob.caffeine.ai";

// The IC network URL (or local replica URL for development).
const IC_URL: string = import.meta.env.VITE_IC_URL ?? "https://icp-api.io";

const IS_LOCAL = IC_URL.includes("127.0.0.1") || IC_URL.includes("localhost");

// ── Agent + Actor ─────────────────────────────────────────────────────────────

// Create a query-capable agent. For write operations the agent is identical;
// IC agent automatically sends update calls for `#[update]` methods.
const agent = new HttpAgent({ host: IC_URL });

// On local networks, fetch the root key so the agent can verify responses.
if (IS_LOCAL) {
  agent.fetchRootKey().catch(console.warn);
}

/** Typed actor for the example backend canister. */
export const backend = Actor.createActor(idlFactory, {
  agent,
  canisterId: CANISTER_ID,
}) as BackendActor;

// ── Candid types (subset used by the frontend) ────────────────────────────────

export interface BlobInfo {
  hash: string;
  name: string;
  size: bigint;
  content_type: string;
  created_at: bigint;
}

export interface CreateCertificateResult {
  method: string;
  blob_hash: string;
}

// The Actor type mirrors the Candid interface.
// If you run `npm run generate` after deploying, the generated declarations
// will replace this manual type with a fully-typed one.
export interface BackendActor {
  _immutableObjectStorageCreateCertificate(hash: string): Promise<CreateCertificateResult>;
  set_blob_info(hash: string, name: string, size: bigint, contentType: string): Promise<void>;
  list_blobs(): Promise<BlobInfo[]>;
  delete_blob(hash: string): Promise<void>;
}

// ── SHA-256 helper ────────────────────────────────────────────────────────────

/**
 * Compute the SHA-256 hash of a file and return it as "sha256:<hex>".
 * Uses the Web Crypto API — available in all modern browsers.
 */
export async function sha256File(file: File): Promise<string> {
  const buffer = await file.arrayBuffer();
  const hashBuffer = await crypto.subtle.digest("SHA-256", buffer);
  const hashArray = Array.from(new Uint8Array(hashBuffer));
  const hashHex = hashArray.map((b) => b.toString(16).padStart(2, "0")).join("");
  return `sha256:${hashHex}`;
}
