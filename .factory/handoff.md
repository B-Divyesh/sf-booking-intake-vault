# Private Intake — repair handoff

## Result: PASS locally; deployment triggered by push

This repair addresses every finding in the independent report for candidate
`8b0817b637eeb6daa7bb4b4ead864e975e11e45c`.

## Repairs

- Direct client routes now receive a real **200** application-shell response:
  `/`, `/book`, `/admin`, `/privacy`, `/terms`, and `/worker/<token>`. Static
  assets still use the static-file service and missing assets remain 404. This
  replaces Axum's `not_found_service(index)`, which returned the index body
  with a 404 status.
- Booking form route markers now use `#725914` on `#F4EBD8` (5.62:1), replacing
  the verifier-reported 3.83:1 `#8B733C` contrast failure.
- The runtime image accepts the factory `BUILD_SHA` build argument and exports
  it as `BUILD_SHA`, so `/health` identifies the deployed source commit.
- Added `Strict-Transport-Security: max-age=31536000; includeSubDomains` to the
  application response policy. The production edge must preserve this header.

## Regression coverage

- Rust integration regression creates an isolated shell and verifies 200 plus
  HTML content type for every supported client route, HSTS, and a 404 for a
  missing asset.
- Playwright regression verifies all standard direct client documents return
  200 with no browser console errors, confirms the worker-document response,
  and asserts `/health` returns its configured build identity.
- Playwright axe now audits both the landing page and the configured booking
  form; serious and critical violations must be empty.

## Verification evidence (2026-08-28)

From a clean `npm ci` (74 packages, zero audit vulnerabilities):

```sh
npm test
npm run build
cargo fmt --check
git diff --check
npm audit --audit-level=high
```

Results:

- `npm test` passed: 3 Vitest tests, 5 Rust tests, and 5 Chromium Playwright
  tests. Browser coverage includes server-enforced worker redaction, direct
  deep links, title/one-h1/main/hero-alt/console checks, both axe audits, and
  390 × 844 no-horizontal-overflow.
- `npm run build` produced `frontend/dist` and
  `target/release/booking-intake-vault`. Production assets: JS 76,909 bytes
  (28.87 KB gzip), CSS 24,036 bytes (6.12 KB gzip), mobile hero 29,756 bytes,
  desktop hero 80,936 bytes.
- `cargo fmt --check`, `git diff --check`, and `npm audit --audit-level=high`
  passed. The project has no separate lint/typecheck script; Vite/Svelte
  production compilation completed cleanly.
- Native release-server probe with `BUILD_SHA=repair-local`: all six supported
  client routes returned `200 text/html; charset=utf-8`; a missing asset
  returned 404; `/health` returned `{"build":"repair-local","status":"ok"}`;
  unconfigured public form access returned 404. CSP, nosniff, DENY frame,
  no-referrer, permissions, no-store, and HSTS response policies were present.
- Offline/update smoke: after service-worker activation on `/book`, a
  service-worker-controlled offline reload rendered the intentional “The
  booking desk is unavailable” state without saved booking data.
- Desktop keyboard smoke: Tab reaches the skip link first with a 3 px focus
  outline. A 390 px reduced-motion browser had no horizontal overflow and
  matched `prefers-reduced-motion: reduce`.
- Privacy smoke: the existing end-to-end booking exercised server-side worker
  redaction; the worker brief contained permitted address/access values but not
  the client name or `PRIVATE BILLING NOTE`.

## Build and deployment

The artifact remains a Rust/Axum + SQLite container serving the Vite/Svelte
build on `PORT` (default 8080). The root `Dockerfile` remains multi-stage and
non-root. The factory must pass the source commit as `--build-arg BUILD_SHA`;
the runtime `/health` then returns that exact value. Push the recorded repair
commit on `main` to trigger the configured container deployment, then confirm
the public `/health` build field equals that commit and recheck direct links.

### Live deployment evidence

- Repair commit `3a356064e6cc15de68471d9fb71a13f5f80913f7` was pushed to `main`.
- Factory ACR build `sf-booking-intake-vault:3a356064e6cc` succeeded with image
  digest `sha256:d375055c1a72bf4593a5fa92e737598e952f0c44cc75f64a8db3f268ca29186f`.
- The configured Container App and
  `https://booking-intake-vault.sociobot.in/health` returned that exact build
  SHA. Public `/`, `/book`, `/admin`, `/privacy`, and `/terms` each returned
  HTTP 200 after rollout.

## Known limitation

Docker CLI is not installed in this worker, so the container image could not
be built locally. Native release compilation and the release binary's route,
identity, security-policy, browser, offline, mobile, accessibility, and
privacy checks all passed.
