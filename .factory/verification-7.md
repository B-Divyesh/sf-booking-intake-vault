# Repair 7 verification record — Private Intake

**Result: PASS for the recorded product defect.**

Verified on 2026-09-05 from a fresh clone of implementation commit
`0ad29bd64650c88b3f80e6f1e02b94d1d29c86c5` and against
`https://booking-intake-vault.sociobot.in`.

## Recorded defect

Verification 6 failed because `npm run test:e2e -- --grep @claim:zero-config`
started Rust before the ignored `frontend/dist` artifact existed. `test:e2e`
now builds the web client before starting Playwright. From a clean clone with no
`frontend/dist`, the exact command built the client and passed its observable
API, rendered-page, accessibility, and console assertions.

Every one of the 17 manifest commands then passed separately. Aggregate results
were 3 Vitest, 19 Rust, and 20 Playwright tests, plus clean Svelte checking,
formatting, strict Clippy, production build, high-severity npm audit, and diff
checks.

## Live result

The implementation is deployed as revision
`sf-booking-intake-vault--0000029`. `/health` returns the full implementation
SHA. The app remains single-replica with Azure Files mounted at `/data`, and the
public-form digest remained identical across a revision restart.

Fresh desktop and phone contexts identified the job, audience, and sample
action before scrolling. The sample banner, realistic separated views, reset,
same-origin traffic, unchanged real form, keyboard focus, reduced motion,
mobile layout, axe, offline reload, routes, headers, 429/Retry-After, 413 policy,
artifact hashes, and Lighthouse budgets passed. A deliberate unknown page
returned the expected designed HTTP 404.

Full evidence and command details are in `.factory/handoff.md` and
`/work/.evidence/`.

## External dependency

The advertised US$29 one-time Route pass is still gated by verified licenses,
but its Sociobot checkout is not registered and currently returns 404. The
separate billing operator can use `/work/.evidence/billing-offer.json`. No paid
capability was removed or made free.
