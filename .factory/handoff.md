# Private Intake — verification 4 handoff

## Result: FAIL

Candidate `bc34c201a3b242f6328f3f14c0dbaa578c742801` was independently
verified on 2026-08-28 against
`https://booking-intake-vault.sociobot.in`. The deployment is the requested
candidate, but it must not be accepted or released as complete.

The fresh verifier report is [verification-4.md](verification-4.md). It found
these release blockers:

- Production `/api/session` says `configured:false`; `/api/form/public` is
  unavailable, so the hosted booking form cannot accept an intake or deliver a
  worker brief.
- A 25-request live login burst produced no 429 at all; local 429 responses
  omit the mandatory `Retry-After` header and the limiter does not use
  ingress `X-Forwarded-For` identity.
- Manager authentication is local passphrase/cookie, not the required
  Sociobot Entra External ID authority if the manager sign-in is in scope.
- Local configured mobile Lighthouse reported Performance 69, LCP 2,854.938
  ms, and CLS 0.9125 (accessibility 100); it misses the stated performance
  targets.

All local automated checks passed (`npm ci`, `npm run check`, format/clippy,
`npm test` with 3 Vitest + 11 Rust + 10 Playwright tests, exact `npm run
build`, and high-severity npm audit). The local product flow itself was
exercised successfully with temporary data: setup, validation recovery,
server-enforced worker redaction, export, and deletion. Desktop/390 px,
keyboard focus, reduced motion, axe, PWA offline reload, privacy request
policy, response headers, and bundle budgets also passed. The detailed report
contains exact evidence and retest steps.

## Required next steps

1. Provision and preserve the production vault through the controlled owner
   bootstrap path.
2. Fix proxy-aware rate limiting and add `Retry-After` to 429 responses.
3. Resolve the manager-auth Entra requirement and configured-booking mobile
   performance regression.
4. Redeploy and request a new independent verification.

---

## Superseded repair claim

This repair addresses the release blocker in verifier report commit
`f03517874c96d5937dc4c565642a9666c5de82c4` for candidate
`84adef21c835e59314f6060be5947d6cc16a9107`.

Production is deployed from repair commit
`efddc8165abc515e030a574732e45b95b42f92f4`; `/health` returns that exact
SHA. The product remains the Rust/Axum + SQLite backend and Svelte/Vite client
served from one container.

## Repairs

- Provisioned the production vault through the existing owner-only bootstrap
  path. Public setup remains disabled in production; the bootstrap passphrase
  is an Azure-managed secret and is not recorded in this repository, logs, or
  this handoff.
- Mounted a dedicated Azure Files `/data` volume and limited this SQLite
  single-tenant service to one replica. Direct SQLite locking on Azure Files
  is not reliable, so the runtime now uses local SQLite and restores/snapshots
  the committed vault database to `/data/booking-intake-vault-local.db` after
  successful API requests. This keeps SQLite locks local while preserving the
  configured vault across revisions.
- Added controlled-bootstrap idempotence, repeated-migration, and durable
  snapshot/restore regression coverage. Transient migration locks also use a
  bounded retry and busy timeout.
- Removed expected offline `ERR_INTERNET_DISCONNECTED` noise by keeping API
  requests out of an offline shell reload. The worker/UI now presents its
  intentional recovery state without a browser console error.
- Preloaded the responsive hero image for LCP, and bumped the service-worker
  shell cache to `private-intake-shell-v3` so clients receive the repaired
  entrypoint after a release.

## Verification evidence

Clean local install and release checks passed:

```sh
npm ci                                  # 79 packages, 0 vulnerabilities
npm test                                # 3 Vitest + 11 Rust + 10 Chromium passed
npm run check                           # Svelte: 0 errors, 0 warnings
npm run build                           # Vite dist + optimized Rust release binary
cargo fmt --all -- --check              # passed
cargo clippy --all-targets --all-features -- -D warnings  # passed
npm audit --audit-level=high             # 0 vulnerabilities
git diff --check                         # passed
```

The browser suite includes desktop and 390 px coverage, keyboard/focus and
axe checks, worker redaction, configured booking, manager detail/delete/form
states, paid-limit API enforcement, deep links, mobile target sizing, service
worker control, and the offline-console regression. Local mobile Lighthouse:
Performance 99, Accessibility 100, Best Practices 100, SEO 100; LCP 1,944 ms
and CLS 0.

Live checks at `https://booking-intake-vault.sociobot.in`:

- `/health` is HTTP 200 with build `efddc8165abc515e030a574732e45b95b42f92f4`.
- `/api/session` is `configured:true`, `authenticated:false`, and
  `setup_allowed:false`; the public form is available with eight fields and no
  visibility metadata.
- A non-customer QA booking completed login (200), booking submission (201),
  one-hour worker assignment (200), server-redacted worker brief, and deletion
  (200). The worker payload included the permitted QA address and omitted both
  manager-only markers.
- A deliberately new container revision restored `configured:true`; the
  durable snapshot on the mounted share was 65,536 bytes after the check.
- The factory URL verifier found a 598 ms desktop load, no console errors,
  title/lang/one `h1`/`main`, and no missing image alt text. Supported direct
  routes `/`, `/book`, `/admin`, `/privacy`, `/terms`, and `/worker/...` all
  returned 200.
- Live 390 px `/book` had no horizontal overflow or console errors, showed
  the configured form, and had zero serious/critical axe violations.

## Known gaps / operations

- The production manager bootstrap secret is intentionally not disclosed. It
  is retained as an Azure Container App secret so the owner can sign in; public
  setup stays unavailable.
- No paid test license was available, so the positive live billing verification
  branch was not charged. Server and browser regressions cover the denial path,
  while the cached positive server branch remains unit-tested.
- The worker environment has no Docker CLI. The factory ACR build of the root
  multi-stage Dockerfile succeeded for the deployed image.
