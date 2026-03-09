# Automation options for setup and tests

This doc compares ways to automate prerequisites, build, and test runs for the example.

## What we want to automate

| Step | Today | Can automate? |
|------|------|----------------|
| Install Rust, dfx, mops, Node | Manual (README) | Partially: version managers or Docker |
| Install PocketIC binary | Manual download | Yes: script can download by platform |
| Build rust-backend WASM | `cargo build --target wasm32-unknown-unknown` | Yes: trivial to script |
| Build motoko-backend | `mops install && dfx build` | Yes: trivial to script |
| Rust unit tests | `cargo test` in rust-backend | Yes |
| Canister tests (PocketIC) | `cargo test` in tests/ | Yes, once PocketIC is present |
| Deploy to IC | `scripts/setup.sh` | Already scripted (needs env + tools) |

So **everything from “build + test” can be fully automated**. **Tool installation** (Rust, dfx, mops, Node) can be automated only by adding a layer (version manager, Docker, or install script).

---

## Option A: Python script + uv

**Idea:** One script (e.g. `scripts/run_tests.py`) that:

1. Checks for `cargo`, `dfx`, `mops`, and **PocketIC** (or downloads PocketIC into `.tools/` by platform).
2. Builds both backends, runs unit tests, runs canister tests.

**Pros:**

- Single entrypoint: `uv run python scripts/run_tests.py` (or `./scripts/run_tests.py` with system Python).
- uv gives a pinned Python and deps (e.g. `requests` for downloading) with no global install.
- No new ecosystem beyond Python; easy to add logic (e.g. GitHub API for latest PocketIC).
- Does **not** install Rust/dfx/mops — those stay “install once from README” or via another tool.

**Cons:**

- Still need to install Rust, dfx, mops, Node manually (or via mise/Docker).

**Verdict:** Best **first step**: automate “download PocketIC if missing + build + run all tests” with minimal surface. Good for CI and for “I already have the tools, just run everything.”

**Implemented:** `scripts/run_tests.py` (stdlib only; no uv required). It pins PocketIC to the same major version as the tests crate (11.x) so the server binary is compatible. From the example root:

```bash
python3 scripts/run_tests.py
```

It checks for `cargo`, `dfx`, `mops`; ensures PocketIC is present (downloads into `.tools/` by platform if missing); builds both backends; runs unit and canister tests.

---

## Option B: mise (or asdf)

**Idea:** Add `mise.toml` (or `.mise.toml`) with Rust and Node. Run `mise install` then a script for build + test. Optionally a custom plugin or script for dfx/mops.

**Pros:**

- Reproducible tool versions; one command to get Rust + Node.
- Fits polyglot repos; many IC devs already use mise/asdf.

**Cons:**

- dfx and mops may not have first-class plugins; might need “install dfx/mops manually” or a small wrapper script.
- PocketIC is still a binary download — same script as in Option A.
- Adds a dependency: users must install mise.

**Verdict:** Good **if** you want to pin Rust/Node and are okay documenting “install mise, then run `mise install`”. Combine with the same Python (or shell) script for PocketIC + build + test.

---

## Option C: Docker

**Idea:** Dockerfile that installs Rust, dfx, Node, mops, PocketIC; entrypoint runs the test script.

**Pros:**

- Fully reproducible; no local tool install; great for CI.
- “Zero local install” for contributors who prefer containers.

**Cons:**

- Heavier; requires Docker; image build can be slow.
- Slightly more maintenance (base image, versions).

**Verdict:** Best for **CI** and for “run in a container” workflows. Overkill for “run tests on my machine” if we only need a single script.

---

## Option D: Makefile / just / task

**Idea:** `make test` (or `just test`) that runs: build rust, build motoko, unit tests, canister tests. Does **not** install tools or PocketIC.

**Pros:**

- Simple; no new runtime; familiar to many.
- Complements Option A: script installs PocketIC and could call `make test`, or user runs `make test` after installing PocketIC once.

**Cons:**

- Does not automate tool or PocketIC install.

**Verdict:** Nice **addition** to Option A: one command for “build + test” assuming tools exist; script handles PocketIC and can invoke make.

---

## Recommendation

1. **Add a single automation script** (Python with uv, or bash) that:
   - Ensures PocketIC is available (download by platform into `.tools/` if missing; set `POCKET_IC_BIN`).
   - Builds rust-backend and motoko-backend.
   - Runs Rust unit tests and canister tests.
   - Single entrypoint: e.g. `./scripts/run_tests.sh` or `uv run python scripts/run_tests.py`.

2. **Keep tool install (Rust, dfx, mops, Node) in the README** as the default path. Optionally add a **mise.toml** later if you want one-command tool install.

3. **Use Docker only if** you need CI or a “run in container” story; the same script can run inside the container.

So: **max automation with minimal surface = script (Python + uv or bash) for PocketIC + build + test**. Optionally layer mise for tools and/or Docker for CI.
