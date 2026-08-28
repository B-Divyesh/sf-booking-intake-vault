# Private Intake — repair handoff

## Result: PASS

All release-blocking findings in verifier report commit
`13cfc6ce4145cd73173073a9e1e032e1720f951a` for candidate
`827582799f19138a0e7c542db355f71f4ae8ea1a` were repaired and covered by
regression tests. The application remains a Svelte/Vite frontend served by the
Rust Axum/SQLite container.

## Repairs

- Added server-owned Route pass enforcement. `PUT /api/form` now requires a
  license verified against Sociobot for 9–12 fields, and worker assignments
  over 48 hours require the same verification. Verdicts are cached by token
  hash for at most 24 hours; raw tokens are neither persisted nor logged.
- Closed the first-visitor takeover path in production. `POST /api/setup` is
  disabled whenever `APP_ENV=production`; an empty production database can be
  initialized only through the platform-secret `INITIAL_ADMIN_PASSPHRASE` and
  optional owner metadata. The service still starts with only `PORT`.
- Added server validation for ISO dates, times, and plausible phone values.
  Duplicate form IDs are rejected as 422 before the existing form transaction
  begins, so SQLite no longer turns malformed input into a 500.
- Replaced low-contrast paper-surface accent text in worker, booking-detail,
  delete-dialog, form-routing, and Route-pass states with palette-consistent
  dark inks. Expanded mobile link targets to at least 44 × 44 CSS px.
- Added an original, self-hosted SVG favicon and advanced the service-worker
  cache to `private-intake-shell-v2`.
- Expected unavailable public-form and worker-ticket states now use explicit
  200-state envelopes, avoiding browser console errors while retaining clear
  error UI. Unknown application and API routes still return 404.
- Added `npm run check` with `svelte-check` and exact browser/API regressions
  for every verifier finding.

## Verification evidence

Clean dependency and full local gates:

```sh
npm ci                                  # 79 packages, 0 vulnerabilities
npm test                                # 3 Vitest + 8 Rust + 9 Chromium passed
npm run check                           # 0 errors, 0 warnings
cargo fmt --all -- --check              # passed
cargo clippy --all-targets --all-features -- -D warnings  # passed
npm audit --audit-level=high             # 0 vulnerabilities
npm run build                            # Vite + optimized Rust release passed
git diff --check                         # passed
```

The browser suite covers landing, booking, manager detail, completed status,
native delete dialog, form-routing choice hints, Route pass, worker brief,
invalid worker state, desktop, 390 × 844 mobile, keyboard focus, axe, and clean
console behavior. All tested states had zero serious/critical axe violations;
all visible mobile links measured at least 44 × 44 px; dialog focus entered on
“Keep booking”; the first keyboard target was the skip link with a visible
3 px brass outline.

Release assets remain within budget: JS 77,624 bytes (29.07 KB gzip), CSS
24,519 bytes (6.21 KB gzip), mobile hero 29,756 bytes, and desktop hero 80,936
bytes. Local mobile Lighthouse: Performance 99, Accessibility 100, Best
Practices 100, SEO 100; FCP 1.4 s, LCP 1.7 s, TBT 120 ms, CLS 0.

The release binary started with only `PORT` and generated its default database.
A production-mode smoke initialized from a supplied secret, reported
`configured:true`/`setup_allowed:false`, rejected public setup with 401, and
served the public form. A 300-request health burst with concurrency 100
returned 300/300 HTTP 200 in 1.118 s. The service worker controlled the page,
updated without a waiting worker, used only `private-intake-shell-v2`, and an
offline `/book` reload showed its explicit unavailable state without private
data.

## Live deployment evidence

The factory ACR build succeeded, the container app was updated, and production
was initialized with an Azure-managed `initial-admin-pass` secret (the value is
not in the repository, logs, or this handoff). Live checks at
`https://booking-intake-vault.sociobot.in` found:

- `/health` returned repair-code commit
  `1dfb6e66f11355fc8731c8643eb1683eef8c774c`; all supported routes returned
  200 and unknown client/API routes returned 404.
- `/api/session` returned `configured:true`, `authenticated:false`, and
  `setup_allowed:false`; the public form exposed all 8 default fields.
- Configured E2E: login 200, booking 201, 48-hour assignment 200, worker brief
  contained the permitted address and zero private markers, deletion 200.
- Direct 9-field mutation without a license returned 402; unauthenticated
  production setup returned 401.
- Desktop and 390 px browser checks had no console errors, horizontal overflow,
  serious/critical axe findings, or third-party requests. Authenticated manager
  detail/dialog and the live worker brief also had zero serious/critical axe
  findings.
- `index.html`, hashed JS, hashed CSS, `favicon.svg`, and `sw.js` matched the
  local production artifacts byte for byte by SHA-256.
- HSTS, CSP, DENY framing, nosniff, no-referrer, Permissions-Policy, document
  `no-store`, immutable hashed-asset caching, and rejected untrusted CORS
  preflight were all observed live.

Run the factory smoke with:

```sh
/opt/fleet/lib/verify-url.sh https://booking-intake-vault.sociobot.in /tmp/private-intake-evidence
```

## Known gaps / next steps

- No paid test purchase token was available, so the positive live Sociobot
  checkout-to-server-verification path was not charged or exercised. A cached
  verified-token unit test covers the positive authorization branch; live and
  browser tests cover denial without a license.
- Docker is unavailable in the worker container, so local `docker build` was
  not run. The factory ACR built the same multi-stage Dockerfile successfully.
- The factory container configuration currently provides revision-local
  SQLite storage. Before accepting durable customer data across future
  revisions, operations should attach persistent `/data` storage and keep this
  single-tenant SQLite service at one replica.
