# Private Intake

Private Intake collects one field-service booking and creates two records: a
complete manager record and a smaller worker brief. The Rust service removes
manager-only answers before it returns any worker response.

Try the isolated sample at `/demo` or
`https://booking-intake-vault.sociobot.in/demo`. It needs no account and does
not change real bookings.

## What it includes

- A hosted eight-question booking form that is ready on first start
- Field labels for `Worker sees` and `Manager only`
- A manager board protected by Sociobot Microsoft Entra External ID
- Server-built worker briefs with expiring, revocable links
- A configured deletion date, immediate deletion, and spreadsheet-safe CSV
- An optional US$29 one-time Route pass for 12 questions and longer links
- An offline demo reload, mobile layouts, and keyboard focus states

Calendar sync, payment collection, emergency dispatch, and CRM features are
outside this version.

## Architecture

Svelte 5 and Vite build the client into `frontend/dist`. Axum serves that client
and a same-origin JSON API. SQLite stores the workspace, form, bookings,
response visibility snapshots, the stable Entra owner ID, and hashed worker
tokens. Demo workspaces stay in a separate in-memory map for 24 hours and use
only `demo:private-intake:` keys in browser session storage.

The browser signs managers in through the Sociobot Entra External ID authority.
The server validates the issuer, RS256 signature, JWKS key, audience, tenant,
and stable `oid` claim before every manager operation. The first valid identity
claims an unowned vault; later identities receive 403.

## Run locally

Requirements: Node 22+, npm, Rust stable, and Chromium for Playwright 1.58.2.

```sh
npm ci
npm run build:web
PORT=8080 cargo run
```

Open `http://localhost:8080`. The public form and demo work immediately. The
manager redirect URI must be registered with Sociobot Entra to sign in outside
the automated test environment.

## Test and build

```sh
npm run check
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
npm test
npm run build
npm audit --audit-level=high
```

`npm test` runs Vitest, Rust integration tests, and Playwright browser tests.
The claim commands are listed in [`.factory/claims.json`](.factory/claims.json).
`npm run test:e2e` builds the web client first, so each browser claim works in
a clean checkout without a pre-existing `frontend/dist` directory.
The production build creates `frontend/dist` and
`target/release/booking-intake-vault`.

## Runtime configuration

The container starts with only `PORT` and creates a usable default workspace.
All other settings are optional overrides:

- `PORT` — listener port, default `8080`
- `DATABASE_URL` — SQLite URL, default `sqlite://private-intake.db?mode=rwc`
- `DATABASE_BACKUP_PATH` — durable snapshot destination when a volume is mounted
- `BUILD_SHA` — value returned by `/health`
- `RUST_LOG` — structured log filter, default `info`
- `INITIAL_BUSINESS_NAME`, `INITIAL_TIMEZONE`, `INITIAL_REGION`, and
  `INITIAL_DELETION_DAYS` — first-boot workspace labels and retention
- `ENTRA_TENANT_ID`, `ENTRA_TENANT_SUBDOMAIN`, and `ENTRA_CLIENT_ID` — optional
  overrides for the built-in Sociobot External ID settings
- `BILLING_BASE` — optional Sociobot billing API override for tests

Production uses one replica and an Azure Files mount at `/data`; see
[`.factory/deployment.md`](.factory/deployment.md). The app restores its local
SQLite copy from that mount at startup and snapshots successful API changes.

## Privacy and operations

The public form never receives field-visibility metadata. Worker queries select
only answers marked `worker`, and assignment tokens are stored as hashes.
Documents and APIs use `no-store`; hashed static assets use immutable caching.
The app loads no analytics, third-party fonts, or third-party runtime scripts.
License verification contacts only the Sociobot API after a license is present.
See `/privacy` and `/terms` in the running application.

Every endpoint is rate limited by the first `X-Forwarded-For` address. The
general allowance is 120 requests per minute, identity checks allow 20, and
writes allow 60. A limited response includes `429` and `Retry-After`.

## Deployment

The factory deploys the root `Dockerfile`; this repository does not manage DNS
or billing registration. The image runs as a non-root user on `PORT`, and
`/health` returns the supplied build SHA.

## License

MIT — see [LICENSE](LICENSE).
