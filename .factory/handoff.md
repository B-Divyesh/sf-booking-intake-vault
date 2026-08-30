# Private Intake — verification 5 handoff

## Result: FAIL

Candidate `e482a8076bca5568cd8b80d47479abdd2f975e28` was independently
verified on 2026-08-30 against
`https://booking-intake-vault.sociobot.in`. Do not release it as complete.

The full evidence is in [verification-5.md](verification-5.md).

## Release blockers

- `.factory/claims.json` is missing, so the mandatory claim-test gate cannot
  run. Claim-like landing and README statements are unlisted.
- The first screen uses a metaphor headline and “Set up your vault”; there is
  no one-click “Try it with sample data” action. `/demo` returns 404, and
  `.factory/demo.md` and `.factory/copy-audit.md` are missing.
- The exact live candidate is unconfigured: `/api/session` reports
  `configured:false` and `/api/form/public` reports `available:false`. The
  hosted product cannot accept a booking or create a worker brief. This is
  fresh evidence that the prior durable-deployment PASS did not persist.
- Manager sign-in uses a local passphrase/cookie, not the required Sociobot
  Entra External ID authority `sociobotcustomers.ciamlogin.com`.
- Public client values beginning with `=` are exported verbatim to CSV, which
  permits spreadsheet formula injection when a manager opens the export.
- Live mobile `/book` has CLS 0.825–0.867 and Lighthouse Performance 75 because
  its large loading skeleton collapses into the unavailable state.

Required metadata/discovery and SPA focus handling are also incomplete:
`robots.txt`, `sitemap.xml`, `/demo`, canonical/Open Graph/Twitter metadata,
the standard footer build/Factory attribution, designed 404, and route-change
heading focus/live announcement are absent.

## What passed

- `npm ci`, `npm run check`, Rust formatting, strict Clippy, `npm test`, exact
  `npm run build`, locked release build, npm high-severity audit, and diff check.
- Tests: 3 Vitest, 16 Rust, and 12 Playwright tests passed.
- A configured isolated local flow passed validation recovery, 24-hour
  retention, manager authorization, server redaction, 48-hour free-link
  boundary, worker access, export, deletion/revocation, keyboard-only booking,
  390 px layout, dialog focus, and snapshot restore after restart.
- Live rate limiting allowed 20 login attempts/minute for one client; request
  21 returned 429 with `Retry-After: 59`. A live 300-request health smoke at
  concurrency 100 returned 300 × 200.
- Desktop and 390 px route checks had one `h1`, `main`, `lang="en"`, no
  horizontal overflow, no console/page errors, and zero serious/critical axe
  findings. Focus, reduced motion, same-origin request privacy, secure headers,
  immutable asset caching, service-worker update, and offline reload passed.
- Local/live JS, CSS, and service-worker hashes match. Bundles are within
  budget: 78,379-byte JS, 24,942-byte CSS, 29,756-byte mobile hero.
- Live landing Lighthouse scored 99/100/100/100 with LCP 1,576 ms and CLS 0;
  configured local `/book` scored 97/100/100/100 with LCP 1,965 ms and CLS 0.

## Verification commands

```sh
npm ci
npm run check
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
npm test
npm run build
cargo build --release --locked
npm audit --audit-level=high
```

Docker is unavailable in the verifier container, so the image itself was not
rebuilt. No product code was changed; only this handoff and the new independent
verification report were written.

## Next steps

Implement the claims/demo/plain-words contracts, restore durable production
configuration, adopt Sociobot Entra, neutralize CSV formulas, stabilize the
unavailable booking layout, and complete required metadata/routing. Redeploy
and request a fresh independent verification including a cleaned-up live
booking-to-worker flow.
