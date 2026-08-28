# Verification report 2 — Private Intake

**Result: FAIL**

Independently verified on 2026-08-28 against candidate commit
`827582799f19138a0e7c542db355f71f4ae8ea1a` and
`https://booking-intake-vault.sociobot.in`.

The prior deployment concern is resolved: production `/health` returns the
exact candidate SHA, all supported client routes return 200, and every fetched
static artifact is byte-for-byte identical to the clean candidate build. The
candidate still fails the acceptance contract for the defects below.

## Release-blocking defects

### High — serious contrast failures in the worker and manager workflows

Independent `@axe-core/playwright` coverage of states omitted by the supplied
tests found `color-contrast` violations at **serious** impact:

- Worker brief: `Worker ticket` is 1.70:1 and the status is 1.46:1 on the paper
  surface.
- Manager booking detail: the `Complete` status is 1.46:1.
- Delete confirmation: `Permanent deletion` is 2.40:1.
- Form routing: the choice-list hint is 2.48:1.
- Route pass: the pass eyebrow is 1.56:1.

Each is small text and requires at least 4.5:1. The supplied Playwright suite
audits only the landing and booking-form states, so its passing axe assertions
do not cover these failures. This directly violates the required zero
serious/critical axe baseline and affects the worker brief central to the
success measure.

### High — paid Route pass limits are enforced only in the browser

With an authenticated free vault and no license token, direct same-origin API
requests unlocked both advertised paid capabilities:

```text
PUT /api/form with 9 fields                         -> 200 {"ok":true}
GET /api/form/public after that                     -> 9 fields
POST /api/bookings/<id>/assign expires_hours=336   -> 200 (14-day link)
```

The UI hides these controls until the Sociobot license check succeeds, but the
backend accepts up to 12 fields and 336-hour links without receiving or
verifying a license. Any signed-in free user can bypass the US$29 unlock with
browser developer tools.

### High — the live product is unconfigured and publicly claimable

Fresh production evidence:

```text
GET /api/session     -> {"configured":false,"authenticated":false,"workspace":null}
GET /api/form/public -> 404
```

Consequently `/book` shows “The booking desk is unavailable” and logs a 404
resource error instead of accepting bookings. The unauthenticated, one-time
`POST /api/setup` route remains open, so the first Internet visitor can claim
the sole production manager vault. I did not mutate production by claiming it.
The live deployment therefore does not currently perform the researched job.

## Other defects

### Medium — server accepts malformed typed values

A booking with `appointment_date: "not-a-date"` and
`contact_number: "not a phone"` returned 201 and was persisted. Required,
length, email, and select validation work, but date input is trusted after the
browser boundary. The backend contract requires validation at the edge.

### Medium — mobile click targets miss the 44 × 44 px contract

At 390 px, measured visible targets included the home/brand link at 147 × 32,
the “See the privacy route” link at 214 × 25, footer Privacy at 50 × 21, footer
Terms at 41 × 21, and legal email links at 19 px tall. Layout had no horizontal
overflow, but these targets do not meet the supplied mobile accessibility and
design contract.

### Low — normal landing load requests a missing favicon

`GET /favicon.ico` returns 404. Lighthouse records this as a console error,
contrary to the no-console-errors gate. Direct `/book` and invalid worker-link
loads also log their expected 404 API responses as console errors.

### Low — duplicate field IDs produce a server error

`PUT /api/form` with two otherwise-valid fields sharing an ID returned 500
after a SQLite uniqueness error. The transaction rolled back and the prior
form survived, but invalid client input should be rejected as 422 without an
internal database error.

## Passing evidence

### Clean install, tests, checks, and exact build

- Detached clean worktree at the candidate SHA; Node 22.23.2, npm 10.9.8,
  rustc/cargo 1.98.0.
- `npm ci`: 74 packages audited, zero vulnerabilities.
- `npm test`: 3 Vitest, 5 Rust, and 5 Chromium Playwright tests passed.
- `npm run build`: exact Vite production build and optimized Rust binary passed.
- `cargo fmt --all -- --check`, `cargo clippy --all-targets --all-features --
  -D warnings`, `npm audit --audit-level=high`, and `git diff --check` passed.
  There is no separate repository typecheck/lint script.
- Docker is not installed in the verifier container, so the image itself could
  not be rebuilt. The native release binary starts with only `PORT`, creates
  its default SQLite database, and serves successfully.

### End-to-end behavior and privacy

- Exercised setup boundaries (name, passphrase, timezone, region, 1–90 day
  retention), repeat setup, bad login, public form, booking submission,
  manager list/detail, redaction preview, assignment, status, CSV export,
  token rotation, logout, deletion, and recovery/error states.
- Missing required, unlisted select, 2,001-byte answer, invalid status, and
  worker expiry 0/337 returned 422; a request over 64 KB returned 413.
- Public form JSON contained no visibility metadata. Manager detail retained
  `PRIVATE-CLIENT-8275` and `PRIVATE-BILLING-8275`; preview and worker JSON
  contained only worker-marked address, schedule, job, and access facts.
- Changing the current form visibility did not alter the existing response
  snapshot. A booking ID could not be used as a worker token; rotating a link
  invalidated the old token; deleting the booking invalidated its live token.
  Raw worker tokens were absent from SQLite.
- Session and worker data survived a release-server restart. Twenty concurrent
  valid booking writes all returned 201 and persisted; 300 concurrent health
  requests all returned 200.
- Production cookies are `HttpOnly; SameSite=Strict; Secure`; unauthenticated
  manager form/list/export returned 401. An untrusted CORS preflight returned
  405 with no allow-origin header. Runtime logs contained no booking IDs or
  worker tokens.

### Browser, offline, deployment, and performance

- Desktop and 390 × 844 mobile checks found one `<h1>`, one `<main>`, `lang=en`,
  appropriate titles, no overflow, and no page exceptions across landing,
  booking, admin, legal, worker, empty, and error states.
- Keyboard first focus is the skip link with a visible 3 px brass outline;
  native dialog focus moved to “Keep booking,” Escape closed it, and normal
  controls were reachable by Tab. Reduced-motion media matched at 390 px and
  collapsed animation/transition durations.
- The service worker controlled the page, `registration.update()` completed
  with no waiting worker, cache `private-intake-shell-v1` was active, and an
  offline `/book` reload rendered the explicit unavailable/retry state without
  exposing saved booking data.
- Landing runtime made no third-party requests. CSP limits resources to self
  and the documented Sociobot billing API; there are no analytics, CDN fonts,
  or third-party scripts.
- HTTP redirects to HTTPS. Production sends HSTS, CSP, DENY framing, nosniff,
  no-referrer, permissions policy, and `no-store` on documents/API. Hashed
  assets use `public, max-age=31536000, immutable`; `sw.js` is `no-store`.
- Production `/health` returned the exact candidate SHA. `/`, `/book`,
  `/admin`, `/privacy`, `/terms`, and `/worker/nonexistent` returned 200.
  SHA-256 matched for `index.html`, JS, CSS, both WebP images, and `sw.js`.
- Build sizes: JS 76,909 bytes (28.87 KB gzip), CSS 24,036 bytes (6.12 KB
  gzip), mobile hero 29,756 bytes, desktop hero 80,936 bytes; all stated
  budgets pass.
- Lighthouse mobile landing metrics were Performance 100, Accessibility 100,
  Best Practices 96, SEO 100; LCP 1.511 s, TBT 54.5 ms, CLS 0. The browser tab
  crashed during the final full-page screenshot after writing the report, so
  treat these as indicative. Lighthouse's landing-only accessibility score
  does not cover the serious authenticated/worker axe findings above.

## Retest requirements

Fix every serious axe violation and add coverage for worker/admin/dialog/pass
states; enforce paid entitlements server-side; configure production through a
controlled owner flow; validate typed values at the API edge; enlarge mobile
targets; and remove expected-load console errors. Then rerun the full clean
build, privacy matrix, live identity/artifact match, and configured live E2E.
