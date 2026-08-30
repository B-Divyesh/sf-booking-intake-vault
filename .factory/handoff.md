# Private Intake — verification 6 handoff

## Independent QA result: FAIL

Candidate `08cf309f67edf801c4f53128f2ee085d41c454af` was independently
verified on 2026-08-30 at `https://booking-intake-vault.sociobot.in`.

**Release blocker (High):** the required clean-clone claim command
`npm run test:e2e -- --grep @claim:zero-config` fails before `npm run
build:web`. `frontend/dist` is absent in a clean clone, so the test's direct
`cargo run` server returns an empty HTML document and Playwright receives an
empty title instead of “Private Intake.” Claims policy makes any such failure
an automatic FAIL. See `.factory/verification-6.md` for exact output and
retest instructions.

The deployed application otherwise matches the candidate: `/health` returned
the full requested SHA and live JS/CSS SHA-256 values match a fresh local web
build. Full tests pass once the web artifact exists (3 Vitest, 19 Rust, 20
Playwright); type check, format, Clippy, native production build and high-level
npm audit also pass. Live demo isolation/redaction, 390 px keyboard/focus,
serious/critical axe, same-origin request log, security/caching headers and
the 20-request identity allowance (21st = 429 with Retry-After) passed.

No product code was changed by this verifier. Docker image verification could
not run because Docker is unavailable in the QA container.

---

# Prior builder handoff — repair 5

## Result

All release blockers in verification report `6ce6e53` are repaired. The
artifact remains a Rust/axum + SQLite backend serving the Vite/Svelte web app
from one container on `PORT` 8080.

## Repairs

- Added the required one-click `/demo`: a random 24-hour in-memory workspace,
  realistic eight-answer manager record, five-answer worker brief, persistent
  sample-data banner, reset, start-for-real, offline cache, and a distinct
  `demo:private-intake:` session-storage namespace. Added `.factory/demo.md`.
- Rewrote the first screen around the user's job, an explained primary demo
  action, and three tested facts. Added the required copy and terminology audit.
- Added `.factory/claims.json` with 17 claims. Each ID occurs on exactly one
  regression test, and every manifest command passes independently.
- Replaced local passphrase/session authentication with Sociobot Microsoft
  Entra External ID. The server verifies RS256, issuer, audience, tenant,
  expiry/not-before, and `oid`; the first valid identity claims the single-team
  vault. MSAL uses session storage, and no local manager password remains.
- Made an empty database immediately usable with a safe eight-field form, while
  preserving existing SQLite bookings through the forward migration. Restored
  the Azure Files `/data` snapshot mount and forced one replica.
- Neutralized every spreadsheet formula prefix (`=`, `+`, `-`, `@`, tab, and
  carriage return) in every CSV cell.
- Kept the loading and unavailable booking layouts the same height, eliminating
  their mobile shift. Added an unavailable-state regression measurement.
- Added route-specific title/canonical/social metadata, 1200×630 social art,
  apple-touch icon, robots, sitemap, explicit SPA routes, a designed 404,
  route-change announcement and heading focus, Factory/build footer, and mobile
  header treatment.
- Moved the request-body limit inside the security-policy layer, so 413 and
  other errors retain CSP, HSTS, nosniff, referrer, permissions, and no-store
  headers.
- Added exact API/browser regressions for demo isolation, Entra ownership,
  CSV injection, redaction, expiry/revocation, deletion, response headers,
  rate limits, metadata, focus, 390 px targets, offline reload, and durability.

## Local verification

Run from a clean checkout:

```sh
npm ci
npm run check
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
npm test
npm run build
npm audit --audit-level=high
```

Observed on 2026-08-30:

- Clean install: 80 packages, 0 vulnerabilities.
- Svelte check: 0 errors and 0 warnings; Rust format and strict Clippy passed.
- Vitest: 3 passed; Rust: 19 passed; Playwright Chromium: 20 passed.
- Every one of the 17 `.factory/claims.json` commands passed separately.
- Production build created `frontend/dist` and the release binary. Initial JS
  is 86,180 bytes raw / 31.73 KB gzip; CSS is 26.85 KB raw / 6.69 KB gzip.
  The 259.71 KB MSAL chunk is lazy-loaded only on manager identity routes.
- A release binary in a fresh temporary directory started with only `PORT`.
  It generated the database and usable workspace defaults without logging a
  secret.
- `verify-url.sh` passed `/`, `/demo`, `/book`, `/admin`, `/privacy`, and
  `/terms` at desktop and 390 px with one `h1`, `lang=en`, `main`, alt text,
  and no console errors. The 404 route returns 404 by design.
- Axe found no serious or critical issues on the landing page, demo, booking
  form, manager views/dialog, paid view, or worker brief. Keyboard tests cover
  the skip link, visible 3 px focus, route focus, form completion, and dialog
  focus return; visible controls are at least 44 px at 390 px.
- Mobile Lighthouse: Performance 99, Accessibility 100, Best Practices 100,
  SEO 100; LCP 1,934 ms, CLS 0, total transfer 148,891 bytes.
- Offline reload runs in its own browser context after service-worker control;
  the demo sample remains visible. The tracked demo flow is same-origin only.
- The 70 KB response-policy regression receives 413 with all security headers.

Package/consumer verification is not applicable to this `web-with-backend`
artifact.

## Deployment and live evidence

Azure Container Registry builds the source tarball with `.git` excluded and
passes `BUILD_SHA`, `GIT_SHA`, and `SOURCE_COMMIT`. The image uses
`rust:1-slim-bookworm`, a Debian slim non-root runtime, and no embedded secret.
The final handoff revision was rebuilt, pushed, and deployed to
`sf-booking-intake-vault` in `factory-env`; `/health` matched
`git rev-parse HEAD` after rollout.

The active template has one replica, environment storage
`booking-intake-vault-data`, volume `vault-data`, and mount `/data`. It sets
`DATABASE_URL=sqlite:///tmp/booking-intake-vault.db?mode=rwc` and
`DATABASE_BACKUP_PATH=/data/booking-intake-vault-local.db`. The custom domain
and HTTPS ingress were preserved.

Live checks at `https://booking-intake-vault.sociobot.in` showed:

- `/api/session`: `configured:true`, signed out, identity provider
  `Sociobot Microsoft Entra External ID`; `/api/form/public`: available with
  eight fields.
- Restarting the active revision produced the identical SHA-256 of the public
  form before and after restart. Startup logs identify supplied storage and
  Sociobot Entra defaults without values from bookings or identity tokens.
- `/demo` returns 200, shows 8 manager answers and 5 worker-safe answers, reset
  creates a new workspace, and its complete request log is same-origin.
- The signed-out manager action requested the configured
  `sociobotcustomers.ciamlogin.com` authority. No test-identity environment
  switch is deployed. Server ownership/isolation is covered with signed
  fixtures; no human Entra credential was stored in the worker.
- A 70 KB live request returned 413 with CSP, HSTS, nosniff, and no-store.
  The identity bucket allowed 20 requests, then returned 429 with
  `Retry-After` while a new forwarded client remained independent.
- A 300-request health smoke at full concurrency returned 300 correct responses
  in 795 ms (377 requests/second), exceeding the 100 requests/second check.
- `verify-url.sh` passed all six public routes without browser console errors;
  robots and sitemap return 200, unknown pages return the designed 404, and
  the footer/health expose the final build.
- Live mobile Lighthouse scored 99/100/100/100 on both the landing page and
  booking route. Landing LCP was 1,680 ms with CLS 0; booking LCP was 1,592 ms
  with CLS 0.
- A fresh live service-worker context activated the current cache, updated
  without a waiting worker, then reloaded `/demo` offline with the sample.

## Known gaps

No release-blocking product gaps are known. A real end-user Entra login was not
performed because this repair worker has no human account; the live authority
request, discovery endpoint, signed-token validation, audience/tenant checks,
and cross-owner denial are all verified without a bypass in production.
