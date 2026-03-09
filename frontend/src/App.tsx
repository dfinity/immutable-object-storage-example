import { useCallback, useEffect, useRef, useState } from "react";
import {
  backend,
  CANISTER_ID,
  GATEWAY_URL,
  sha256File,
  type BlobInfo,
} from "./canister";

// ── Types ─────────────────────────────────────────────────────────────────────

type UploadStatus = "idle" | "hashing" | "certifying" | "uploading" | "saving" | "done" | "error";

interface UploadState {
  status: UploadStatus;
  message: string;
  progress: number; // 0–100
}

// ── Helpers ───────────────────────────────────────────────────────────────────

function formatBytes(bytes: bigint): string {
  const n = Number(bytes);
  if (n === 0) return "0 B";
  const units = ["B", "KB", "MB", "GB", "TB"];
  const i = Math.floor(Math.log(n) / Math.log(1024));
  return `${(n / Math.pow(1024, i)).toFixed(1)} ${units[i]}`;
}

function formatDate(nanos: bigint): string {
  return new Date(Number(nanos / 1_000_000n)).toLocaleString();
}

function blobDownloadUrl(canisterId: string, hash: string): string {
  // The gateway serves blobs at /blob/<owner>/<hash>
  return `${GATEWAY_URL}/blob/${canisterId}/${hash}`;
}

// ── Upload logic ──────────────────────────────────────────────────────────────

async function uploadFile(
  file: File,
  onProgress: (state: UploadState) => void
): Promise<void> {
  if (!CANISTER_ID) {
    throw new Error(
      "VITE_CANISTER_ID is not set. " +
        "Set it in frontend/.env or deploy the backend first (dfx deploy)."
    );
  }

  // Step 1: compute SHA-256 hash
  onProgress({ status: "hashing", message: "Computing SHA-256 hash…", progress: 10 });
  const hash = await sha256File(file);

  // Step 2: create upload certificate on the canister
  onProgress({ status: "certifying", message: "Creating upload certificate…", progress: 25 });
  const cert = await backend._immutableObjectStorageCreateCertificate(hash);

  if (cert.method !== "upload" || cert.blob_hash !== hash) {
    throw new Error(`Unexpected certificate: ${JSON.stringify(cert)}`);
  }

  // Step 3: upload to the storage gateway
  onProgress({ status: "uploading", message: "Uploading to storage gateway…", progress: 40 });
  const uploadUrl = blobDownloadUrl(CANISTER_ID, hash);

  const response = await fetch(uploadUrl, {
    method: "PUT",
    headers: {
      "Content-Type": file.type || "application/octet-stream",
      "Content-Disposition": `attachment; filename="${encodeURIComponent(file.name)}"`,
    },
    body: file,
  });

  if (!response.ok) {
    const text = await response.text().catch(() => response.statusText);
    throw new Error(`Gateway upload failed (${response.status}): ${text}`);
  }

  // Step 4: save display metadata on-chain
  onProgress({ status: "saving", message: "Saving file metadata…", progress: 85 });
  await backend.set_blob_info(
    hash,
    file.name,
    BigInt(file.size),
    file.type || "application/octet-stream"
  );

  onProgress({ status: "done", message: "Upload complete!", progress: 100 });
}

// ── Components ────────────────────────────────────────────────────────────────

function DropZone({
  onFiles,
  disabled,
}: {
  onFiles: (files: File[]) => void;
  disabled: boolean;
}) {
  const [dragging, setDragging] = useState(false);
  const inputRef = useRef<HTMLInputElement>(null);

  const handleDrop = useCallback(
    (e: React.DragEvent) => {
      e.preventDefault();
      setDragging(false);
      if (disabled) return;
      const files = Array.from(e.dataTransfer.files);
      if (files.length > 0) onFiles(files);
    },
    [disabled, onFiles]
  );

  return (
    <div
      className={`drop-zone ${dragging ? "drop-zone--active" : ""} ${disabled ? "drop-zone--disabled" : ""}`}
      onDragOver={(e) => { e.preventDefault(); if (!disabled) setDragging(true); }}
      onDragLeave={() => setDragging(false)}
      onDrop={handleDrop}
      onClick={() => !disabled && inputRef.current?.click()}
    >
      <input
        ref={inputRef}
        type="file"
        multiple
        style={{ display: "none" }}
        onChange={(e) => {
          const files = Array.from(e.target.files ?? []);
          if (files.length > 0) onFiles(files);
          e.target.value = "";
        }}
      />
      <span className="drop-zone__icon">☁</span>
      <p className="drop-zone__label">
        {disabled ? "Uploading…" : "Drop files here or click to select"}
      </p>
      <p className="drop-zone__hint">
        Files are content-addressed — the SHA-256 hash is stored on-chain.
      </p>
    </div>
  );
}

function UploadProgress({ state }: { state: UploadState }) {
  if (state.status === "idle") return null;

  const isError = state.status === "error";
  const isDone = state.status === "done";

  return (
    <div className={`upload-progress ${isError ? "upload-progress--error" : ""}`}>
      <div className="upload-progress__bar">
        <div
          className="upload-progress__fill"
          style={{ width: `${state.progress}%`, transition: "width 0.3s ease" }}
        />
      </div>
      <p className={`upload-progress__message ${isDone ? "upload-progress__message--done" : ""}`}>
        {state.message}
      </p>
    </div>
  );
}

function BlobCard({
  blob,
  canisterId,
  onDelete,
}: {
  blob: BlobInfo;
  canisterId: string;
  onDelete: (hash: string) => void;
}) {
  const [deleting, setDeleting] = useState(false);
  const downloadUrl = blobDownloadUrl(canisterId, blob.hash);

  const handleDelete = async () => {
    if (!confirm(`Delete "${blob.name || blob.hash}"? This cannot be undone.`)) return;
    setDeleting(true);
    try {
      await backend.delete_blob(blob.hash);
      onDelete(blob.hash);
    } catch (e) {
      alert(`Delete failed: ${e}`);
      setDeleting(false);
    }
  };

  return (
    <div className="blob-card">
      <div className="blob-card__icon">{fileIcon(blob.content_type)}</div>
      <div className="blob-card__info">
        <p className="blob-card__name" title={blob.name || blob.hash}>
          {blob.name || <span className="blob-card__unnamed">unnamed</span>}
        </p>
        <p className="blob-card__meta">
          {blob.content_type && <span>{blob.content_type} · </span>}
          {formatBytes(blob.size)} · {formatDate(blob.created_at)}
        </p>
        <p className="blob-card__hash" title={blob.hash}>{blob.hash}</p>
      </div>
      <div className="blob-card__actions">
        <a
          className="btn btn--secondary"
          href={downloadUrl}
          target="_blank"
          rel="noopener noreferrer"
          download={blob.name || undefined}
        >
          Download
        </a>
        <button
          className="btn btn--danger"
          onClick={handleDelete}
          disabled={deleting}
        >
          {deleting ? "Deleting…" : "Delete"}
        </button>
      </div>
    </div>
  );
}

function fileIcon(contentType: string): string {
  if (contentType.startsWith("image/")) return "🖼";
  if (contentType.startsWith("video/")) return "🎬";
  if (contentType.startsWith("audio/")) return "🎵";
  if (contentType.includes("pdf")) return "📄";
  if (contentType.includes("zip") || contentType.includes("tar")) return "📦";
  return "📁";
}

// ── Main App ──────────────────────────────────────────────────────────────────

export default function App() {
  const [blobs, setBlobs] = useState<BlobInfo[]>([]);
  const [loading, setLoading] = useState(true);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [upload, setUpload] = useState<UploadState>({ status: "idle", message: "", progress: 0 });

  const loadBlobs = useCallback(async () => {
    setLoadError(null);
    try {
      const result = await backend.list_blobs();
      setBlobs(result);
    } catch (e) {
      setLoadError(`Failed to load blobs: ${e}`);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => { loadBlobs(); }, [loadBlobs]);

  const handleFiles = useCallback(async (files: File[]) => {
    for (const file of files) {
      setUpload({ status: "hashing", message: `Processing ${file.name}…`, progress: 5 });
      try {
        await uploadFile(file, setUpload);
        // Refresh the blob list after each successful upload.
        await loadBlobs();
      } catch (e) {
        setUpload({ status: "error", message: `Upload failed: ${e}`, progress: 0 });
        return;
      }
    }
    setTimeout(() => setUpload({ status: "idle", message: "", progress: 0 }), 3000);
  }, [loadBlobs]);

  const handleDelete = useCallback((hash: string) => {
    setBlobs((prev) => prev.filter((b) => b.hash !== hash));
  }, []);

  const isUploading = upload.status !== "idle" && upload.status !== "done" && upload.status !== "error";

  return (
    <div className="app">
      <header className="header">
        <h1 className="header__title">
          ☁ Caffeine Object Storage
        </h1>
        <p className="header__sub">
          Immutable, content-addressed file storage on the Internet Computer.{" "}
          {CANISTER_ID && (
            <span className="header__canister">Canister: {CANISTER_ID}</span>
          )}
        </p>
      </header>

      <main className="main">
        {!CANISTER_ID && (
          <div className="alert alert--warning">
            <strong>Configuration required:</strong> Set <code>VITE_CANISTER_ID</code> in{" "}
            <code>frontend/.env</code>, or run <code>dfx deploy</code> in the backend directory
            first (dfx writes the ID automatically).
          </div>
        )}

        <section className="section">
          <h2 className="section__title">Upload files</h2>
          <DropZone onFiles={handleFiles} disabled={isUploading || !CANISTER_ID} />
          <UploadProgress state={upload} />
        </section>

        <section className="section">
          <div className="section__header">
            <h2 className="section__title">Stored files</h2>
            <button className="btn btn--ghost" onClick={loadBlobs} disabled={loading}>
              {loading ? "Loading…" : "Refresh"}
            </button>
          </div>

          {loadError && <div className="alert alert--error">{loadError}</div>}

          {!loading && blobs.length === 0 && !loadError && (
            <p className="empty">No files stored yet. Upload one above!</p>
          )}

          <div className="blob-list">
            {blobs.map((blob) => (
              <BlobCard
                key={blob.hash}
                blob={blob}
                canisterId={CANISTER_ID}
                onDelete={handleDelete}
              />
            ))}
          </div>
        </section>
      </main>

      <footer className="footer">
        <p>
          Files are stored at{" "}
          <a href={GATEWAY_URL} target="_blank" rel="noopener noreferrer">
            {GATEWAY_URL}
          </a>
          . Hashes are anchored on-chain via the{" "}
          <a
            href="https://dashboard.internetcomputer.org/canister/72ch2-fiaaa-aaaar-qbsvq-cai"
            target="_blank"
            rel="noopener noreferrer"
          >
            Cashier canister
          </a>
          .
        </p>
      </footer>
    </div>
  );
}
