# Verification report 4 — Private Intake

**Result: FAIL**

Independently verified on 2026-08-28 from a clean checkout of candidate
`bc34c201a3b242f6328f3f14c0dbaa578c742801` against
`https://booking-intake-vault.sociobot.in`.

The deployment identity and published release assets match the candidate, but
the live service is not configured and cannot perform the product's central
job: accept a booking and create a least-privilege worker brief. It also fails
the mandatory server rate-limit contract. Do not release this candidate as
PASS.

## Release-blocking defects

### High — production has no configured vault, so no hosted intake can be submitted

Fresh, non-mutating production evidence:

```text
GET /health           -> 200 {"build":"bc34c201a3b242f6328f3f14c0dbaa578c742801","status":"ok"}
GET /api/session      -> 200 {"configured":false,"authenticated":false,"setup_allowed":false,"workspace":null}
GET /api/form/public  -> 200 {"available":false,"error":"The booking desk has not been configured yet."}
```

At both 1280 px and 390 x 844 px, a fresh visit to `/book` showed only
“The booking desk is unavailable” and its retry button. It had no client form
and therefore could not collect an appointment, retain a deletion date, or
produce a worker brief. Production setup correctly rejects public claims, so
the deployment owner must provision the controlled bootstrap and retain the
initialized database on durable storage before retest. No live setup, booking,
or other customer-like mutation was performed during this verification.

This fails the smallest useful product and its success measure.

### High — rate limiting does not satisfy the required live or response contract

The server-side requirement is a 429 response *with `Retry-After`*, keyed to
the first `X-Forwarded-For` hop behind ingress. The implementation instead
keys its small in-process window to `ConnectInfo`'s socket peer and its 429
response has no `Retry-After` header.

- Fresh live burst: 25 rapid `POST /api/login` requests with a deliberately
  invalid non-secret passphrase all returned **422**; no 429 threshold was
  observed and consequently no `Retry-After` was returned.
- Fresh local burst: the same endpoint returned 16 × 401 and then 9 × 429
  (the effective threshold was request 17 because four earlier setup/login
  attempts shared the one-minute IP window). The first 429 included security
  headers and JSON but no `Retry-After` header.

This is a security and availability acceptance failure. Apply a proxy-aware,
all-endpoint limiter and return a meaningful `Retry-After` on every 429, then
retest through the factory ingress.

### Medium — manager authentication is not the required Sociobot Entra External ID sign-in

The application implements a local manager passphrase, an Argon2 hash, and a
cookie session. There is no request to, configuration for, or authority at
`sociobotcustomers.ciamlogin.com`. If the manager vault's sign-in is within
the stated sign-in requirement, this does not meet it. Clarify the intended
authentication model or use the required Sociobot Microsoft Entra External ID
tenant exclusively.

### Medium — primary configured booking page misses the stated mobile performance targets

Fresh mobile Lighthouse against a locally configured `/book` flow (standard
Lighthouse mobile throttling) reported Performance **69**, Accessibility 100,
Best Practices 100, SEO 100; LCP **2,854.938 ms** and CLS **0.9125**. This
misses LCP < 2.5 s and CLS < 0.1. A first measurement while the release build
was compiling was worse (LCP 3,991.769 ms), so the repeated post-build
measurement is the reported value. The product needs a stable initial booking
layout and a repeatable performance retest.

## Passing evidence

### Clean checkout, installation, checks, tests, and production build

- Workspace was clean at the requested candidate SHA before the report files
  were added. `npm ci` installed 78 packages and reported zero vulnerabilities.
- `npm run check` passed: Svelte check 0 errors, 0 warnings.
- `cargo fmt --all -- --check` and
  `cargo clippy --all-targets --all-features -- -D warnings` passed.
- `npm test` passed: Vite compilation, 3 Vitest tests, 11 Rust tests, and all
  10 Playwright Chromium tests. The suite's final `test-results/.last-run.json`
  records `status: passed`.
- Exact production `npm run build` passed and produced `frontend/dist` plus
  `target/release/booking-intake-vault`.
- `npm audit --audit-level=high` and `git diff --check` passed. Docker is not
  installed in this verifier container, so its multi-stage image was not built
  here. The released binary did start and serve `/health` with only `PORT`
  set, from an isolated temporary working directory.

### Local end-to-end and privacy checks

On a fresh temporary SQLite database, independently:

1. Configured a vault using the minimum one-day retention boundary.
2. Confirmed the public form exposed eight fields and no `visibility` metadata.
3. Confirmed missing required values returned 422 with an actionable first
   error, then submitted valid normal booking data.
4. Confirmed an unauthenticated manager-detail request returned 401.
5. Logged in as the manager, assigned a 48-hour worker link, and checked the
   server response. Both QA-only manager markers (client and billing context)
   remained in manager detail and CSV export but were absent from worker JSON;
   permitted site, date, job, and access values remained present.
6. Deleted the booking and confirmed its former worker link returned the
   unavailable/expired envelope.

No customer data or real credentials were used. This demonstrates the
least-privilege filtering itself works locally, but it cannot compensate for
the unconfigured live deployment.

### Browser, accessibility, PWA, privacy, and release identity

- Fresh production Chromium checks at desktop and 390 px saw no console or
  page errors, zero axe serious/critical findings, one `h1`, `main`,
  `lang="en"`, no horizontal overflow, and the visible 3 px brass keyboard
  focus ring. These checks exercised the unavailable state because production
  is unconfigured.
- A fresh configured local booking page at 390 px had zero axe
  serious/critical findings, no horizontal overflow, a Tab-reachable visible
  skip-link focus ring, and reduced-motion transition duration `0.01 ms`.
- The PWA service worker controlled the local page; `registration.update()`
  left no waiting worker, cache `private-intake-shell-v3` was active, and an
  offline reload rendered the deliberate unavailable/retry state without
  console errors.
- Observed runtime requests on the public booking screen were same-origin
  document, assets, `/api/form/public`, and `/api/session`; no analytics,
  third-party font/script, or tracking request appeared. CSP permits only
  same origin plus the documented Sociobot billing API for connections.
- Production sends HSTS, `X-Frame-Options: DENY`, `nosniff`, no-referrer,
  camera/microphone/geolocation Permissions-Policy, CSP with
  `frame-ancestors 'none'`, and `no-store` for documents/API. Hashed assets
  are `public, max-age=31536000, immutable`; `sw.js` is `no-store`. An
  untrusted CORS preflight returned 405 with no allow-origin header.
- Production direct routes `/`, `/book`, `/admin`, `/privacy`, `/terms`, and
  `/worker/no-such-token` returned 200; unknown client/API paths returned 404.
  Production `/health` returned the exact candidate SHA. Local and production
  SHA-256 hashes matched for the built JS, CSS, and service worker.
- Build budgets pass: JS 77,707 bytes (29.09 KB gzip), CSS 24,519 bytes
  (6.21 KB gzip), mobile hero 29,756 bytes, desktop hero 80,936 bytes. They
  are within the stated transfer budgets.

## Retest requirements

1. Initialize the production vault through the controlled deployment path,
   preserve its database across revisions, and demonstrate public booking,
   manager detail, server-redacted worker brief, export, and deletion without
   disclosing credentials or customer data.
2. Implement ingress-aware, all-endpoint rate limiting with 429 plus
   `Retry-After`; record the live observed threshold using the first
   `X-Forwarded-For` hop.
3. Resolve or explicitly re-scope the Entra requirement for manager login.
4. Reduce configured mobile booking LCP below 2.5 s and CLS below 0.1, then
   attach a clean Lighthouse rerun.
