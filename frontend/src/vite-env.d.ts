/// <reference types="vite/client" />

interface ImportMetaEnv {
  readonly VITE_CANISTER_ID?: string;
  readonly VITE_CANISTER_ID_EXAMPLE_BACKEND?: string;
  readonly VITE_STORAGE_GATEWAY_URL?: string;
  readonly VITE_IC_URL?: string;
}

interface ImportMeta {
  readonly env: ImportMetaEnv;
}
