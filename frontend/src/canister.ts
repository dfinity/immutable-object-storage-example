/**
 * canister.ts — IC agent wiring for the example backend canister.
 *
 * This module creates an agent and actor that talk to the example backend
 * canister using the Candid interface defined in rust-backend/src/example_backend.did.
 *
 * For production deployments replace the placeholder values with your own
 * canister ID and network URL.
 */

import { Actor, HttpAgent } from "@icp-sdk/core/agent";
import { idlFactory } from "./declarations/example_backend.did";
import { StorageClient } from "./storage-client";

// ── Configuration ─────────────────────────────────────────────────────────────

export const CANISTER_ID: string =
  import.meta.env.VITE_CANISTER_ID ??
  (import.meta.env.VITE_CANISTER_ID_EXAMPLE_BACKEND as string | undefined) ??
  "";

export const GATEWAY_URL: string =
  import.meta.env.VITE_STORAGE_GATEWAY_URL ?? "https://blob.caffeine.ai";

const IC_URL: string = import.meta.env.VITE_IC_URL ?? "https://icp-api.io";

const IS_LOCAL = IC_URL.includes("127.0.0.1") || IC_URL.includes("localhost");

// ── Agent + Actor ─────────────────────────────────────────────────────────────

export const agent = new HttpAgent({ host: IC_URL });

if (IS_LOCAL) {
  agent.fetchRootKey().catch(console.warn);
}

export const backend = Actor.createActor(idlFactory, {
  agent,
  canisterId: CANISTER_ID,
}) as BackendActor;

// ── Storage Client ────────────────────────────────────────────────────────────

export const storageClient = new StorageClient({
  gatewayUrl: GATEWAY_URL,
  canisterId: CANISTER_ID,
  agent,
});

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

export interface BackendActor {
  _immutableObjectStorageCreateCertificate(hash: string): Promise<CreateCertificateResult>;
  set_blob_info(hash: string, name: string, size: bigint, contentType: string): Promise<void>;
  list_blobs(): Promise<BlobInfo[]>;
  delete_blob(hash: string): Promise<void>;
}
