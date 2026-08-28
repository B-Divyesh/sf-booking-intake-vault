# Verification report 3 — Private Intake

**Result: FAIL**

Independently verified on 2026-08-28 against candidate commit
`84adef21c835e59314f6060be5947d6cc16a9107` and
`https://booking-intake-vault.sociobot.in`.

The earlier build-identity and deep-link failures are resolved: production
`/health` returns the exact candidate SHA, supported SPA routes return 200,
and every deployed release asset fetched below has the same SHA-256 as the
clean candidate build. The candidate nevertheless fails the acceptance
contract because the live product cannot accept a booking.

## Release-blocking defects

### High — the live booking product is unconfigured and cannot be used

Fresh, non-mutating production evidence:

```text
GET /health           -> 200 {"build":"84adef21c835e59314f6060be5947d6cc16a9107","status":"ok"}
GET /api/session      -> 200 {"configured":false,"authenticated":false,"setup_allowed":false,"workspace":null}
GET /api/form/public  -> 200 {"available":false,"error":"The booking desk has not been configured yet."}
POST /api/setup       -> 401 {"error":"Sign in to continue."}
```

At 390 px, a fresh browser visit to `/book` rendered only “The booking desk is
unavailable” and its retry control; it could not show a form or submit an
intake. Production setup is appropriately disabled, so a visitor cannot claim
the vault, but the deployment owner has also not supplied the controlled
bootstrap configuration (or the initialized database is no longer present).
No production booking or setup mutation was made during verification.

This fails the smallest useful product and its central success measure: a
hosted form must collect a client submission and produce a permitted worker
brief. Configure the live owner through `INITIAL_ADMIN_PASSPHRASE` on durable
`/data` storage, verify the initialized public form, and retain the database
across revisions before retest.

## Other defects

### Medium — measured mobile LCP exceeds the stated 2.5 s budget

An idle, clean local mobile Lighthouse run against the candidate production
build reported Performance 95, Accessibility 100, Best Practices 100, SEO
100, CLS 0, TBT 0 ms, and LCP **2,624.915 ms**. This is just over the
performance-skill target of LCP < 2.5 s. The initial JS, CSS, fonts, and hero
asset size budgets pass; optimize/retest the landing LCP under the agreed
throttle before calling the release fully compliant.

### Low — an intentional offline API miss still emits browser console errors

The service worker correctly served the application shell offline and `/book`
rendered its explicit unavailable/retry state without exposing data. Chromium
also logged two `net::ERR_INTERNET_DISCONNECTED` resource errors for the public
form fetch. The online desktop/mobile paths had no console or page errors.
Handle or cache this expected offline fetch if the no-console-errors gate is
intended to include offline loads.

## Passing evidence

### Clean checkout, checks, and build

- Cloned the repository into a new detached checkout at exactly
  `84adef21c835e59314f6060be5947d6cc16a9107`; it was clean before testing.
- `npm ci` completed (79 packages audited; zero vulnerabilities).
- `npm run check` completed with Svelte check: 0 errors and 0 warnings.
- `npm test` passed: Vite production compilation, 3 Vitest tests, Rust tests,
  and all 9 Chromium Playwright tests. The following `npm run build` completed
  the exact Vite plus `cargo build --release` production build and produced
  `frontend/dist` and `target/release/booking-intake-vault`.
- Additional available checks passed: `cargo fmt --all -- --check`,
  `cargo clippy --all-targets --all-features -- -D warnings`,
  `npm audit --audit-level=high`, and `git diff --check`.
- Docker is not installed in this verifier container, so the multi-stage image
  could not be rebuilt here.

### End-to-end, privacy, validation, and backend checks

- On a fresh local SQLite database, configured a vault with a one-day deletion
  period, submitted a normal eight-field booking, opened the manager record,
  previewed the server-built worker record, created a 48-hour worker link,
  rotated it, exported CSV, and deleted the booking. Manager-only markers
  `PRIVATE-CLIENT-QA` and `PRIVATE-BILLING-QA` remained in the manager/export
  record and were absent from both preview and bearer-token worker JSON;
  permitted site, date, job, and access details remained. Old rotated tokens
  and a guessed booking ID returned the unavailable worker envelope.
- The public form JSON contained no `visibility` metadata. Unauthenticated
  manager detail returned 401. Missing required input, invalid date, invalid
  phone (covered in the browser/API suite), and worker expiry `0` each returned
  422 with actionable errors. Deletion removed the arrival-board record.
- A 100-request `/health` burst at concurrency 25 returned 100/100 HTTP 200.
  The release service started with only `PORT` during the supplied browser
  suite; local health identity was explicitly tested through `BUILD_SHA`.
- Independent Chromium checks at 1280 px and 390 x 844 found no horizontal
  overflow. Axe had zero serious/critical violations on local landing,
  configured booking, manager-detail, and worker-brief screens. Keyboard focus
  reached the skip link and exposed a 3 px brass outline; reduced motion was
  active and transitions reduced to `0.01 ms`.
- The PWA service worker controlled the page, `registration.update()` left no
  waiting worker, and the sole active cache was `private-intake-shell-v2`.
  Offline reload retained only the explicit unavailable state.
- No landing-page outbound requests were observed. CSP limits runtime resource
  connections to same-origin plus the documented Sociobot billing API; there
  are no analytics, CDN fonts, or third-party scripts.

### Deployment, headers, cache, and artifact identity

- Production routes `/`, `/book`, `/admin`, `/privacy`, `/terms`, and
  `/worker/no-such-token` returned 200; unknown client and API paths returned
  404. HTTP redirects to HTTPS.
- Production sends HSTS, `X-Frame-Options: DENY`, `nosniff`, no-referrer,
  camera/microphone/geolocation Permissions-Policy, CSP with `frame-ancestors
  'none'`, and `no-store` for documents/API. Hashed assets are
  `public, max-age=31536000, immutable`; `sw.js` is `no-store`. An untrusted
  CORS preflight was 405 with no allow-origin header.
- Local/deployed SHA-256 pairs matched exactly for `index.html`, JS, CSS, both
  WebP hero variants, `sw.js`, and `favicon.svg`. Built size evidence: JS
  77,624 bytes (29.07 KB gzip), CSS 24,519 bytes (6.21 KB gzip), mobile hero
  29,756 bytes, desktop hero 80,936 bytes — within their stated transfer
  budgets.

## Retest requirements

1. Initialize the deployed production vault through the platform-controlled
   bootstrap flow and demonstrate `configured:true`, a public form, one booking
   submission, authenticated manager view, redacted worker link, and deletion
   without publishing credentials or customer-like data.
2. Confirm persistent `/data` survives the deployment/revision boundary that
   led to the currently empty production state.
3. Re-measure mobile LCP below 2.5 s and, if offline console cleanliness is a
   release gate, eliminate the expected disconnected-fetch console entries.
