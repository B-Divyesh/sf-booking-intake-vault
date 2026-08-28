# Verification report — Private Intake

**Result: FAIL**

Tested on 2026-08-28 against candidate commit
`8b0817b637eeb6daa7bb4b4ead864e975e11e45c` and production URL
`https://booking-intake-vault.sociobot.in`.

## Release-blocking defects

### High — all SPA deep links respond with HTTP 404

Fresh requests to the candidate's local server and to production returned 404
for `/book`, `/admin`, `/privacy`, and `/terms`. The server sends the SPA HTML
body, so the browser can render it, but it retains the 404 response status and
reports a console error. This affects normal direct visits, refreshes and
shared worker links (`/worker/<token>` follows the same fallback path).

Evidence:

```text
https://booking-intake-vault.sociobot.in/        -> 200
https://booking-intake-vault.sociobot.in/book    -> 404
https://booking-intake-vault.sociobot.in/admin   -> 404
https://booking-intake-vault.sociobot.in/privacy -> 404
https://booking-intake-vault.sociobot.in/terms   -> 404
```

Local `curl -D - http://127.0.0.1:8092/book` reproduced `HTTP/1.1 404 Not
Found` with the compiled `index.html` body. Playwright captured
`Failed to load resource: the server responded with a status of 404` on a
direct `/book` load.

### High — serious accessibility violation on the configured booking form

Independent `@axe-core/playwright` against the local production web build
found `color-contrast` at **serious** impact on the booking route. The visible
route-step labels `01`, `02`, and `03` are `#8b733c` on `#f4ebd8`: 3.83:1 at
10.24 px; axe requires 4.5:1. The published CSS hash matches this candidate,
so this is present in the deployed artifact as well.

The candidate's supplied browser test only audits the landing route, which is
why it did not expose this booking-form violation.

## Other defects and deployment findings

### Medium — deployed backend has no candidate build identity

Production `/health` returned:

```json
{"build":"development","status":"ok"}
```

The backend-service contract requires `/health` to return the build SHA. The
static files are an exact candidate match (see below), but the backend cannot
be conclusively tied to this commit from production evidence.

### Low — no HSTS header observed

The HTTPS production response includes CSP, `X-Frame-Options: DENY`, nosniff,
Referrer-Policy, Permissions-Policy, and cache policy, but no
`Strict-Transport-Security` header was returned. This may be an edge policy
rather than application code, but it should be configured before release.

## Passing evidence

- Clean checkout at the tested candidate; `npm ci` completed with zero audit
  vulnerabilities.
- `npm test` passed: 3 Vitest tests, 4 Rust tests, and 3 Playwright tests.
  A direct repeat of `cargo test --quiet` and `npm run test:e2e` passed.
- Exact production build `npm run build` completed; `cargo fmt --check` and
  `git diff --check` passed. No repository lint/type script exists beyond
  these checks.
- Built budgets: JS 76,909 bytes (28.87 KB gzip), CSS 24,020 bytes (6.13 KB
  gzip), 390 px hero 29,756 bytes, desktop hero 80,936 bytes.
- Local Lighthouse mobile run on `/`: Performance 95, Accessibility 100,
  LCP 1.994 s, CLS 0, TBT 237 ms. Lighthouse emitted a post-audit browser-tab
  crash warning after writing its report, so treat the score as indicative,
  not a clean Lighthouse completion.
- Local server-enforced privacy exercise: public form JSON contained no
  visibility metadata; unauthenticated manager API was 401; missing required
  input, invalid worker expiry, invalid status, and a >64 KB request body
  returned 422, 422, 422, and 413 respectively. A submitted record retained
  `PRIVATE-CLIENT-9901` and `PRIVATE-BILLING-9901` for the manager while both
  server preview and worker-token JSON omitted them and retained permitted
  address/job/access values. A guessed booking ID did not work as a worker
  token (404). 300 concurrent `/health` requests succeeded.
- Worker-link persistence/expiry design, deletion/export code paths, and
  browser offline shell were inspected. A localhost service-worker-controlled
  offline reload of `/book` loaded the shell and showed its intentional
  booking-desk-unavailable offline state; it did not expose saved booking data.
- Keyboard smoke on the landing page reached the skip link first; computed
  focus was a visible 3 px solid brass outline. At 390 px the booking page had
  zero horizontal overflow and reduced-motion media was active.
- No outbound runtime requests were observed on the landing page; CSP permits
  only same-origin resources and the documented Sociobot billing endpoint.
  The app has no analytics or third-party font/script request.
- Production asset SHA-256 values exactly matched local candidate outputs for
  `index.html`, `index-DLBMGB3q.js`, `index-pXXTZzBJ.css`, both WebP artwork
  variants, and `sw.js`.

## Constraints / retest

- Production was intentionally unconfigured (`/api/session`:
  `configured:false`), so I did not create a real vault or submit customer-like
  data there. Its public form API correctly returned 404 in that unconfigured
  state.
- Docker is not installed in this verification container, so the Docker image
  could not be independently built. The requested native release build did
  complete.

Retest after correcting the fallback to return 200 for valid client routes,
fixing booking-form contrast, and deploying `BUILD_SHA` so `/health` reports
the candidate commit.
