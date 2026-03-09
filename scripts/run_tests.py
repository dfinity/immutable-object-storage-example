#!/usr/bin/env python3
"""
Automated prep and test run for the immutable-object-storage-example.

- Ensures PocketIC binary is available (downloads by platform if missing).
- Builds rust-backend and motoko-backend.
- Runs Rust unit tests and PocketIC canister tests.

Usage (from immutable-object-storage-example/):
  python3 scripts/run_tests.py

Requires Python 3.10+, and cargo, dfx, mops already installed. See README.
"""

from __future__ import annotations

import gzip
import os
import platform
import shutil
import stat
import subprocess
import sys
import urllib.request

SCRIPT_DIR = os.path.dirname(os.path.abspath(__file__))
EXAMPLE_ROOT = os.path.dirname(SCRIPT_DIR)
TOOLS_DIR = os.path.join(EXAMPLE_ROOT, ".tools")
POCKET_IC_BIN_NAME = "pocket-ic"
# Pin to 11.x to match tests/Cargo.toml pocket-ic = "11.0.0" (server must be >=11, <12)
POCKET_IC_VERSION = "11.0.0"

# (system, machine) -> asset filename
ASSET_MAP = {
    ("Linux", "x86_64"): "pocket-ic-x86_64-linux.gz",
    ("Linux", "aarch64"): "pocket-ic-arm64-linux.gz",
    ("Linux", "arm64"): "pocket-ic-arm64-linux.gz",
    ("Darwin", "x86_64"): "pocket-ic-x86_64-darwin.gz",
    ("Darwin", "arm64"): "pocket-ic-arm64-darwin.gz",
}


def log(msg: str) -> None:
    print(f"  [run_tests] {msg}")


def require_cmd(cmd: str) -> None:
    if not shutil.which(cmd):
        print(f"  [run_tests] ERROR: '{cmd}' not found. Install it first (see README Prerequisites).", file=sys.stderr)
        sys.exit(1)


def ensure_pocket_ic() -> str:
    """Return path to PocketIC binary, downloading if needed. Sets POCKET_IC_BIN in env for child processes."""
    existing = os.environ.get("POCKET_IC_BIN")
    if existing and os.path.isfile(existing) and os.access(existing, os.X_OK):
        log(f"Using PocketIC at POCKET_IC_BIN={existing}")
        return existing

    os.makedirs(TOOLS_DIR, exist_ok=True)
    local_bin = os.path.join(TOOLS_DIR, POCKET_IC_BIN_NAME)
    if os.path.isfile(local_bin) and os.access(local_bin, os.X_OK):
        log(f"Using PocketIC at {local_bin}")
        os.environ["POCKET_IC_BIN"] = local_bin
        return local_bin

    key = (platform.system(), platform.machine())
    asset = ASSET_MAP.get(key)
    if not asset:
        log(f"Unsupported platform: {key}. Set POCKET_IC_BIN to your PocketIC binary and re-run.")
        sys.exit(1)

    log(f"Downloading PocketIC {POCKET_IC_VERSION} ({asset})...")
    url = f"https://github.com/dfinity/pocketic/releases/download/{POCKET_IC_VERSION}/{asset}"
    try:
        with urllib.request.urlopen(url, timeout=60) as resp:
            data = gzip.decompress(resp.read())
    except Exception as e:
        log(f"Download failed: {e}. Install PocketIC manually and set POCKET_IC_BIN.")
        sys.exit(1)

    with open(local_bin, "wb") as f:
        f.write(data)
    os.chmod(local_bin, stat.S_IRWXU)
    os.environ["POCKET_IC_BIN"] = local_bin
    log(f"Installed PocketIC at {local_bin}")
    return local_bin


def run(cmd: list[str], cwd: str, description: str) -> None:
    log(description)
    r = subprocess.run(cmd, cwd=cwd, env=os.environ)
    if r.returncode != 0:
        print(f"  [run_tests] FAILED: {' '.join(cmd)}", file=sys.stderr)
        sys.exit(r.returncode)


def main() -> None:
    print("")
    print("  Caffeine Object Storage — Example: automated build + test")
    print("")

    require_cmd("cargo")
    require_cmd("dfx")
    require_cmd("mops")

    ensure_pocket_ic()

    rust_dir = os.path.join(EXAMPLE_ROOT, "rust-backend")
    motoko_dir = os.path.join(EXAMPLE_ROOT, "motoko-backend")
    tests_dir = os.path.join(EXAMPLE_ROOT, "tests")

    run(
        ["cargo", "build", "--target", "wasm32-unknown-unknown"],
        rust_dir,
        "Building rust-backend WASM...",
    )
    run(
        ["cargo", "test"],
        rust_dir,
        "Rust unit tests...",
    )
    run(
        ["mops", "install"],
        motoko_dir,
        "mops install (motoko-backend)...",
    )
    run(
        ["dfx", "build", "example_backend", "--check"],
        motoko_dir,
        "Building motoko-backend WASM...",
    )
    run(
        ["cargo", "test"],
        tests_dir,
        "Canister tests (PocketIC)...",
    )

    log("All steps completed successfully.")
    print("")


if __name__ == "__main__":
    main()
