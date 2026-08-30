# Private Intake — repair 4 handoff

## Result: PASS

This repair closes every finding in verifier report commit
`813cd131fcae80b8bac8fc12f142dd1aaf7c5b95` for candidate
`bc34c201a3b242f6328f3f14c0dbaa578c742801`. The tested implementation is
commit `e2175e69c5c8ca42f4a86197937e98f7521c8c5e`.

## Repairs

- Restored the existing `booking-intake-vault-data` Azure Files volume at
  `/data`, restored the configured vault snapshot, and constrained this SQLite
  service to one replica. `.factory/deployment.md` now records the stateful
  settings that must survive future deployments.
- Added the optional `MANAGER_PASSPHRASE` platform secret. It initializes or
  rotates the manager's Argon2 hash and revokes old sessions without logging or
  committing the secret. Production holds the value as an Azure Container App
  secret; the public setup route remains closed.
- Replaced handler-only socket-peer throttles with middleware on every API
  route. It keys clients by the first `X-Forwarded-For` hop, applies a 120/min
  API ceiling plus 20/min authentication and 60/min write ceilings, exempts
  only `/health`, and returns a positive `Retry-After` on every 429.
- Removed the booking route's unrelated hero preload and replaced its changing
  loading screen with a stable, reserved form skeleton. The service-worker
  cache is now `private-intake-shell-v4`.
- Clarified the authentication boundary in the product, README, and
  `.factory/auth.md`: this is one deployment-local team vault, not a Sociobot
  user account. Workers receive narrow booking links. A future multi-manager,
  multi-tenant, or Sociobot-account scope must replace the local credential
  with Sociobot Entra and use its stable `oid`.
- Updated the Rust builder to the supported rolling `rust:1-bookworm` image.

## Exact regressions

- Rust tests exercise the first forwarded hop, isolation between forwarded
  clients, 429 plus `Retry-After`, the general all-API ceiling, controlled
  production bootstrap, durable restore, manager-credential rotation/session
  revocation, route-pass enforcement, typed validation, and worker redaction.
- Playwright reproduces a 20-request login allowance followed by 429 and checks
  `Retry-After`; a different first forwarded hop remains allowed.
- Playwright observes layout-shift entries at 390 × 844 and requires CLS below
  0.1. It also asserts that `/book` does not request landing artwork.
- The browser matrix covers desktop and 390 px, axe serious/critical findings,
  keyboard focus, mobile targets, all workflow states, paid API limits, direct
  routes, console errors, service-worker update, and offline reload.

## Clean local verification

```text
npm ci                                      79 packages; 0 vulnerabilities
npm run check                               0 errors; 0 warnings
cargo fmt --all -- --check                  passed
cargo clippy --all-targets --all-features -- -D warnings
                                             passed
npm test                                    3 Vitest + 16 Rust + 12 Chromium passed
npm run build                               Vite dist + optimized Rust binary passed
npm audit --audit-level=high                0 vulnerabilities
git diff --check                            passed
```

The configured release binary started with production settings from an empty
temporary directory. `/health` returned its supplied build identity, the
public form was available, all supported routes returned 200, unknown routes
returned 404, and 300 concurrent health requests returned 300 × 200.

Local mobile Lighthouse on configured `/book`: Performance 99,
Accessibility 100, Best Practices 100, SEO 100, LCP 1,772.781 ms, CLS 0,
TBT 12 ms. Built assets: JS 78,379 bytes (29,214 bytes gzip), CSS 24,942
bytes, mobile artwork 29,756 bytes, desktop artwork 80,936 bytes.

At 1280 px and 390 × 844, every public, booking, manager-login, legal, and
invalid-worker route had one `h1`, a `main`, no horizontal overflow, no console
errors, and zero serious/critical axe findings. The first Tab stopped on the
skip link with a 3 px brass focus outline. Reduced-motion transitions were
0.01 ms. Booking traffic remained same-origin. Offline reload used
`private-intake-shell-v4`, had no waiting worker or console errors, and showed
the deliberate unavailable state without exposing booking data.

## Live deployment evidence

ACR build `ch1cj` produced digest
`sha256:7f35d212eadf5f45e262022469c5d69a25766b5b33127114937ff572551f5324`.
The repaired implementation ran in Container App revision
`sf-booking-intake-vault--0000025`.

- `/health` returned exact build
  `e2175e69c5c8ca42f4a86197937e98f7521c8c5e`.
- `/api/session` returned `configured:true`, `authenticated:false`, and
  `setup_allowed:false`; `/api/form/public` returned an available eight-field
  form with no visibility metadata.
- After a deliberate revision restart, the same build and `configured:true`
  state returned immediately. A manager login using the platform-held secret
  returned 200 after that restart.
- A non-customer QA flow returned 200 login, 201 booking, 200 manager detail,
  200 redaction preview, 200 one-hour assignment, 200 worker brief, 200 CSV
  export, and 200 deletion. The manager marker was present in manager/export
  data and absent from preview/worker data; permitted address data remained.
  Deletion invalidated the worker link. The QA record was removed.
- The report's exact 25-request ingress test returned 20 × 401 followed by
  5 × 429. Request 21 included `Retry-After: 58`; a different first forwarded
  hop still reached authentication and returned 401.
- Live URL verification loaded configured `/book` in 655 ms with no console
  errors, one `h1`, `lang=en`, a `main`, and no missing image alternatives.
- Clean live mobile Lighthouse: Performance 100, Accessibility 100, Best
  Practices 100, SEO 100, LCP 1,390.537 ms, CLS 0, TBT 4.5 ms.
- Desktop and 390 px live checks across `/`, `/book`, `/admin`, `/privacy`,
  `/terms`, and an invalid worker link had no console errors, overflow, or axe
  serious/critical findings. All traffic on those fresh pages was same-origin.
- Live security policy included HSTS, DENY framing, nosniff, no-referrer,
  Permissions-Policy, restrictive CSP, no-store documents/API, and immutable
  hashed assets. Public setup and unauthenticated manager data returned 401;
  an untrusted preflight returned 405 without an allow-origin header.
- Local and live SHA-256 values matched for JS, CSS, and `sw.js`. A 300-request
  live health smoke at concurrency 100 returned 300 × 200.

## Known gaps

No release-blocking or verifier finding remains. No paid live license was
available, so the paid positive path was not charged; its cached-positive unit
test and browser/server denial regressions pass. The checkout remains the
documented Sociobot-hosted flow.
