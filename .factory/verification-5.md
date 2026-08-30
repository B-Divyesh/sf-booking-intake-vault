# Verification report 5 — Private Intake

## Result: FAIL

Independently verified on 2026-08-30 from a clean checkout of candidate
`e482a8076bca5568cd8b80d47479abdd2f975e28` against
`https://booking-intake-vault.sociobot.in`.

The live deployment identifies itself as the requested commit and its JS, CSS,
and service-worker bytes match the local production build. It is nevertheless
not releasable. The required claims manifest and demo do not exist, the first
screen fails the plain-words/demo gate, production has again lost its configured
vault, and manager sign-in does not use the required Sociobot Entra authority.
Additional release findings are a CSV formula-injection boundary and a severe
layout shift on the live booking route.

No product code was modified during this verification.

## Mandatory gate results

### FAIL — claims manifest and claim tests are absent

The first clean-checkout command confirmed `HEAD` was the requested SHA and the
worktree was clean, then attempted to read the mandatory manifest:

```text
$ git rev-parse HEAD
e482a8076bca5568cd8b80d47479abdd2f975e28
$ sed -n '1,240p' .factory/claims.json
sed: can't read .factory/claims.json: No such file or directory
```

There were therefore no manifest-listed claim commands to run through a demo
entry point. This is release-blocking by the claims contract. Claim-like copy
is present but unlisted, including “Server-enforced roles,” “Automatic
deletion,” “No trackers,” server filtering, expiring worker links, CSV export,
and offline behavior.

### FAIL — cold first read and one-click demo

Cold, signed-out desktop and 390 × 844 visits showed:

- What it appears to do: collect client details once, retain private context
  for a manager, and send a smaller job brief to a worker.
- Who it appears to be for: field teams, stated only in the jargon-heavy
  eyebrow “Least-privilege booking for field teams.”
- What it tells the visitor to click first: “Set up your vault.”

This does not pass the required first screen. The headline is the metaphor
“Client details, routed with care,” not the job in plain words; the primary
action has no adjacent explanation of what follows; the three facts omit
offline behavior and price. Most decisively, there is no visible “Try it with
sample data” action. `/demo` returns 404 and `?demo=1` does not enter a demo.
There is no demo banner, reset, sample workspace, or real/demo storage
separation. `.factory/demo.md` and `.factory/copy-audit.md` are also absent.

## Release-blocking defects

### Critical — live production is unconfigured and cannot do the core job

Fresh live responses were:

```text
GET /health          200 {"build":"e482a8076bca5568cd8b80d47479abdd2f975e28","status":"ok"}
GET /api/session     200 {"authenticated":false,"configured":false,"setup_allowed":false,"workspace":null}
GET /api/form/public 200 {"available":false,"error":"The booking desk has not been configured yet."}
```

`/book` renders only “The booking desk is unavailable,” while `/admin` renders
“This vault is awaiting its owner.” Public setup is correctly closed in
production, so a visitor cannot recover. The live service cannot accept an
intake, retain it, let a manager assign it, or produce a worker brief. This
fails the smallest useful product and contradicts the previous handoff's claim
that the durable vault survived a revision restart.

The deployment is not stale: `/health` returns the candidate SHA, and SHA-256
hashes of live/local `index-Ad32kIHR.js`, `index-D0PCf_mc.css`, and `sw.js`
match exactly.

### High — required claims-as-tests and sandbox demo contracts are missing

The absence of `.factory/claims.json`, `/demo`, and `.factory/demo.md` prevents
independent proof of every public promise from a clean, isolated sample. This
is independent of the production configuration loss and is itself an explicit
release blocker.

### High — manager authentication is not Sociobot Entra External ID

The product requires manager sign-in but uses an installation-local passphrase,
Argon2 hash, and `piv_session` cookie. The source, browser traffic, and sign-in
screen contain no authority or request to
`sociobotcustomers.ciamlogin.com`. The repository explicitly documents Entra
as outside v1. That conflicts with the verification contract requiring that a
product with sign-in use the Sociobot Microsoft Entra External ID tenant and
nothing else.

### High — public intake values can become spreadsheet formulas in CSV export

A local configured flow submitted this public client name:

```text
=HYPERLINK("https://example.invalid","QA")
```

The manager export returned it verbatim as the first character of a CSV cell:

```csv
...,Client name,"=HYPERLINK(""https://example.invalid"",""QA"")"
```

CSV quoting does not neutralize spreadsheet formulas. Because the input is
untrusted public form data and managers are expected to open the export, the
export must prefix formula-leading values (`=`, `+`, `-`, `@`, tab, carriage
return) or use another safe encoding. This is CWE-1236 spreadsheet formula
injection.

### Medium — live booking route exceeds the CLS budget

Fresh mobile Lighthouse against live `/book` scored Performance 75,
Accessibility 100, Best Practices 100, SEO 100, with LCP 1,505 ms, TBT 86 ms,
and CLS **0.867**. A separate 390 × 844 `PerformanceObserver` run measured CLS
**0.825** as the reserved booking skeleton collapsed into the much shorter
unavailable state. Both exceed the required CLS < 0.1.

For comparison, the configured local `/book` path scored Performance 97,
Accessibility 100, Best Practices 100, SEO 100, with LCP 1,965 ms, TBT 176 ms,
and CLS 0. The regression is specific to the live unavailable/error path, which
is the route every current visitor receives.

### Medium — required routing and discovery metadata are incomplete

- `/robots.txt`, `/sitemap.xml`, and `/demo` return 404; there is no designed
  404 document.
- No route has a canonical link, Open Graph metadata, or Twitter card.
- There is no apple-touch icon or 1200 × 630 social image declaration.
- Route titles do not use the required “Page — Private Intake” pattern; for
  example `/privacy` is only “Privacy notice.”
- The footer omits “Built by Param Factory” and a build/version identifier.
- Client-side navigation leaves focus on the activated link and provides no
  route-change live announcement; the new `h1` is not focused.

### Low — body-limit errors bypass the normal security-header layer

A 70 KB JSON body correctly returned 413, but that response contained only
`content-type`, `content-length`, and `date`. The normal CSP, HSTS, nosniff,
frame, referrer, permissions, and cache-control headers were absent because the
64 KB body-limit layer returns outside the security-header middleware.

## Passing local functional evidence

The core least-privilege implementation works in an isolated configured local
instance:

1. The service started with one-day retention and exposed eight public fields
   without visibility metadata.
2. Missing required input, malformed phone/date/select values, and a 2,001
   character value returned actionable 422 errors. A valid request returned
   201 with a deletion time 24 hours ahead.
3. Wrong manager credentials returned 401. Correct credentials returned an
   HttpOnly, SameSite=Strict, Secure production cookie. An unauthenticated
   booking-detail request returned 401.
4. Manager detail contained `QA-MANAGER-SECRET`; the server-built preview and
   worker API omitted it and the client name while retaining the permitted
   address/job data.
5. Worker-name and link-expiry boundaries rejected invalid values; 49 hours
   without a pass returned 402 and the free 48-hour boundary succeeded.
6. Invalid status was rejected, valid completion succeeded, immediate delete
   removed the booking, and its former worker link became unavailable.
7. A complete booking was submitted with only Tab, Enter, arrow keys, and text
   entry. Focus was visible with a 3 px ring, including the skip link.
8. The delete dialog moved focus inside itself and Escape returned focus to the
   trigger. Configured 390 px manager/detail screens had no overflow, no
   serious/critical axe findings, and no console errors.

Snapshot persistence also passed locally. After a successful booking, the
working SQLite file and backup were byte-identical. The working file was moved
aside, the release binary restarted, and the backup restored the booking. It
was then deleted; the working and backup files again matched.

The release binary also started and served `/health` plus the landing page from
an isolated directory with only `PORT` set. It logged that the database used a
generated default and did not print secret values.

## Rate limit and concurrency evidence

The repaired rate limiter passes live:

```text
POST /api/login, one fixed X-Forwarded-For client:
requests 1–20  -> 422 (vault unconfigured)
request 21     -> 429, Retry-After: 59
requests 22–25 -> 429, Retry-After: 58–59
```

The observed authentication allowance is **20 requests per 60 seconds per
client**, with a documented general API ceiling of 120/minute and write ceiling
of 60/minute. `/health` is exempt. A fresh live concurrency smoke sent 300
health requests in three batches of 100: all 300 returned 200 with the exact
candidate build identity in 547 ms. The same 300-request local smoke also
returned 300 × 200.

## Accessibility, privacy, PWA, and browser evidence

- Fresh Chromium at 1440 × 900 and 390 × 844 covered `/`, `/book`, `/admin`,
  `/privacy`, `/terms`, and an invalid worker route. Every screen had one `h1`,
  `main`, `lang="en"`, no horizontal overflow, no console/page errors, no
  undersized visible controls, and zero axe serious/critical findings.
- The first Tab exposes the skip link at `top: 8px` with a 3 px brass focus
  outline. The configured booking form was operable by keyboard. Reduced-motion
  styles reduce transition/animation duration to 0.01 ms. At 200% root text
  size, the checked landing and form screens retained their main content and
  had no horizontal overflow.
- Cold landing, booking, manager, legal, and invalid-worker request logs were
  same-origin only. No analytics, third-party font/script, or tracking request
  occurred. The public form omits field visibility. Application logs observed
  during the synthetic flow did not emit booking IDs or worker tokens.
- Documents and APIs return `Cache-Control: no-store`; hashed assets return
  `public, max-age=31536000, immutable`; `sw.js` returns `no-store`. Live
  documents include HSTS, `nosniff`, `DENY`, `no-referrer`, restrictive
  Permissions-Policy, and CSP with `frame-ancestors 'none'`. An untrusted CORS
  preflight returned 405 without `Access-Control-Allow-Origin`.
- The service worker controlled a fresh live `/book`, `registration.update()`
  left no waiting worker, and cache `private-intake-shell-v4` was active. An
  offline reload returned the shell, showed the offline/unavailable recovery
  state, and produced no console error.
- All rendered internal links returned 200; the two legal email links were
  valid `mailto:` links. Unknown client and API paths returned 404.

## Build, checks, and budgets

All repository-provided checks pass despite the acceptance failures:

```text
npm ci                                      passed; 78 packages; 0 vulnerabilities
npm run check                               passed; 0 errors, 0 warnings
cargo fmt --all -- --check                  passed
cargo clippy --all-targets --all-features -- -D warnings
                                             passed
npm test                                    passed
  Vitest                                    3 passed
  Rust                                      16 passed
  Playwright Chromium                       12 passed
npm run build                               passed; frontend/dist + release binary
cargo build --release --locked              passed
npm audit --audit-level=high                passed; 0 vulnerabilities
git diff --check                             passed before report edits
```

Production artifacts are within the static budgets:

- JS: 78,379 bytes raw / 29.33 KB gzip (budget 200 KB initial JS).
- CSS: 24,942 bytes raw / 6.33 KB gzip (budget 50 KB).
- Mobile hero WebP: 29,756 bytes (budget 300 KB).
- Desktop hero WebP: 80,936 bytes.
- Live landing Lighthouse: Performance 99, Accessibility 100, Best Practices
  100, SEO 100; LCP 1,576 ms, CLS 0, TBT 101 ms, total transfer 135,056 bytes.

The live server does not compress JS/CSS (`Content-Encoding` is absent), and
Lighthouse estimates about 66 KiB savings, but the measured transfer remains
within the first-load budget.

The Docker CLI is not installed in this verifier container, so the multi-stage
image could not be rebuilt here. The exact local release build and only-PORT
runtime smoke passed.

## Required retest

1. Add `.factory/claims.json` with one demo-based observable test for every
   page/README claim, then run every listed command from a clean checkout.
2. Add the required one-click `/demo` sandbox, persistent banner/reset/start
   actions, isolated ephemeral data, `.factory/demo.md`, and plain-words first
   screen; add `.factory/copy-audit.md`.
3. Restore and prove durable live vault configuration across a fresh revision
   restart, then perform and clean up a live booking → manager preview → worker
   brief → export → deletion flow.
4. Replace local manager authentication with the required Sociobot Entra
   External ID authority.
5. Neutralize CSV formula-leading values and add a regression test using a
   public-form payload.
6. Keep the unavailable/error `/book` state within CLS < 0.1 and complete the
   required route metadata, 404/discovery files, footer, and focus announcement.
