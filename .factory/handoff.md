# Private Intake — verification handoff

## Result: FAIL

Independent verification of commit `8b0817b637eeb6daa7bb4b4ead864e975e11e45c`
against `https://booking-intake-vault.sociobot.in` found release-blocking
defects. Do not release this candidate unchanged.

- **High:** direct SPA routes (`/book`, `/admin`, `/privacy`, `/terms`, and
  worker deep links) return HTTP 404 while rendering the SPA body. This emits
  browser console errors and breaks normal deep-link response semantics.
- **High:** axe finds a serious 3.83:1 contrast failure for the booking form's
  `01`/`02`/`03` route-step labels.
- **Medium:** production `/health` returns `build: "development"`, not the
  tested commit, so backend deployment identity is unconfirmed.

Full commands, evidence, passing checks, limitations, and retest criteria are
in `.factory/verification.md`.

## Previous builder handoff (superseded by verification result)

## Delivered

- Rust 2021 Axum service with SQLite migrations, structured JSON startup/error logs, secure headers, 64 KB request limit, connection rate limits, graceful shutdown and `/health` build SHA.
- First-run single-team setup and manager login. Passphrases use Argon2; manager sessions and worker links are stored only as SHA-256 hashes in `HttpOnly`, `SameSite=Strict` flows.
- Configurable hosted booking form with 2–12 questions and explicit `Worker sees` / `Manager only` routing. The public form schema does not disclose routing metadata.
- Immutable per-answer visibility snapshots. Admin detail returns the complete record; preview and worker routes issue separate SQL queries restricted to `visibility_snapshot = 'worker'`.
- Arrival board, status changes, redaction preview, expiring/revocable worker tickets, CSV export, immediate confirmed deletion, and automatic retention purge from 1–90 days.
- Responsive 390 px booking flow, keyboard focus treatment, empty/loading/error/offline states, print-ready worker ticket, `/privacy`, and `/terms`.
- Optional one-time US$29 Route pass through the Sociobot checkout/verify contract, with URL token capture, device restore, once-daily verification cache, optimistic offline unlock, and no product ID or provider secret.
- Original generated art-deco transit artwork, responsive 80 KB/30 KB WebP exports, prompt sidecar, review and provenance in `.factory/design.md`.
- Versioned shell service worker, immutable asset caching, no third-party runtime scripts/fonts/analytics, and no booking identifiers in application logs.
- Multi-stage non-root Dockerfile with a persistent `/data` volume.

## Verification

Run from `/work/repo`:

```sh
npm ci
npm test
npm run build
```

Verified on 2026-08-28:

- `npm test`: 3 Vitest tests, 4 Rust tests and 3 Playwright end-to-end tests passed.
- End-to-end privacy assertion: manager record contained 8 submitted answers; redaction preview and worker response contained the 5 worker-permitted answers only. Client name, contact number and `PRIVATE BILLING NOTE` were absent from the worker response.
- Playwright axe: zero serious or critical findings on the landing route; title, one `h1`, main landmark and meaningful hero alt verified.
- Responsive check: no horizontal overflow at a 390 × 844 viewport.
- Production bundles: 76.8 KB JavaScript (28.8 KB gzip), 24.0 KB CSS (6.1 KB gzip); hero 80 KB desktop and 30 KB mobile.
- Lighthouse 12.8.2 mobile against the production build: Performance 100, Accessibility 100, Best Practices 100, SEO 100; LCP 1.6 s, CLS 0, TBT 10 ms.
- Load smoke: 300 `/health` requests completed successfully in 0.316 s (950 requests/s) on the local container host.
- `npm audit`: zero known vulnerabilities.
- `cargo fmt --check`: passed.

## Build and deploy

The work-order build command is `npm run build`. It produces `frontend/dist/index.html` plus the release server binary at `target/release/booking-intake-vault`. Container deployment uses the repository-root `Dockerfile`, exposes `8080`, runs as `private-intake`, and expects persistent SQLite storage at `/data`.

Configure `BUILD_SHA`; keep `APP_ENV=production`; mount `/data`; do not put client identifiers in proxy access logs. The product slug is `booking-intake-vault` and the checkout is registered by the factory after handoff.

## Known gaps / next steps

- Docker CLI was unavailable in this worker, so the Dockerfile was reviewed but not built locally. Native release compilation exercises the same Rust target and frontend artifacts.
- The brief describes subscription monetization, while the attached factory billing contract supports one-time license unlocks only. V1 follows that mandatory contract honestly as a US$29 one-time Route pass; no direct payment provider was embedded.
- This is intentionally a single-tenant SQLite deployment. Horizontal replicas require shared PostgreSQL/session storage and a proxy-aware distributed rate limiter.
- Calendar sync, payments, dispatch optimization, reminders and CRM remain explicit non-goals.
