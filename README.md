# Private Intake

Private Intake is a small-team booking form that turns one client submission into two deliberately different records: a complete manager record and a minimal, expiring worker brief. Field visibility is enforced by the Rust service, not by hiding values in the browser.

It is for plumbers, cleaners, repair teams and other field-service operators who need the site and job facts to travel, while client contact, property, project or commercial context stays private.

## What v1 includes

- First-run single-team vault setup with an Argon2-hashed manager passphrase
- Hosted, configurable intake form with `Worker sees` and `Manager only` fields
- Immutable visibility snapshots on every submitted answer
- Manager arrival board, complete record and server-built redaction preview
- Expiring, revocable worker links backed by hashed tokens
- Automatic 1–90 day deletion, immediate confirmed deletion and CSV export
- Optional US$29 one-time Route pass through the Sociobot billing API
- Privacy/terms pages, offline feedback, mobile layouts and keyboard focus states

Non-goals are calendar sync, payment collection, dispatch optimization and CRM functionality.

## Architecture

The Svelte 5/Vite client is compiled to `frontend/dist`. Axum serves that client and a same-origin JSON API. SQLite stores the single workspace, form configuration, bookings, response visibility snapshots, hashed manager sessions and hashed worker tokens. No booking or worker token is written to application logs.

## Run locally

Requirements: Node 22+, npm, Rust 1.88+, and Chromium for the Playwright version pinned in `package.json`.

```sh
npm ci
npm run build:web
DATABASE_URL='sqlite://private-intake.db?mode=rwc' cargo run
```

Open `http://localhost:8080`. In local development, the first manager to open `/admin` configures the vault. Production disables public setup and must be initialized with the deployment-only bootstrap variables below. Use a persistent database file in production; losing that file loses the vault.

For frontend hot reload, run the API as above and run `npm run dev` separately. Vite proxies API calls to port 8080.

## Test and build

```sh
npm test
npm run build
```

`npm test` runs Vitest utility tests, Rust authorization/data tests, and Playwright end-to-end tests including axe serious/critical checks and the 390 px booking layout. The reproducible production build command is exactly `npm run build`; web artifacts land in `frontend/dist` and the Rust binary in `target/release/booking-intake-vault`.

If Playwright browsers are not already present:

```sh
npx playwright install chromium
```

## Container

```sh
docker build -t private-intake .
docker run --rm -p 8080:8080 -v private-intake-data:/data private-intake
```

Runtime configuration is environment-only:

- `PORT` — listener port, default `8080`
- `DATABASE_URL` — SQLite URL, default in the image is `sqlite:///data/private-intake.db?mode=rwc`
- `APP_ENV=production` — enables `Secure` on manager session cookies
- `BUILD_SHA` — returned by `/health` for deployment identification
- `RUST_LOG` — structured JSON log filter, default `info`
- `INITIAL_ADMIN_PASSPHRASE` — optional production bootstrap secret; when the database is empty, creates the owner workspace without exposing a public claim endpoint
- `INITIAL_BUSINESS_NAME`, `INITIAL_TIMEZONE`, `INITIAL_REGION`, `INITIAL_DELETION_DAYS` — optional bootstrap metadata with safe defaults
- `BILLING_BASE` — optional Sociobot API override for integration testing

The service starts successfully with only `PORT`. For an Internet-facing deployment, supply `INITIAL_ADMIN_PASSPHRASE` as a platform secret before opening the form. Its value is never logged. Once initialized on persistent storage, the bootstrap secret can be removed.

## Privacy and operations

All private filtering happens in SQL on the server. Public form configuration omits visibility metadata. Worker links are bearer secrets: share them only with the assigned worker and issue a new link if one escapes. Paid limits are also enforced by the server using Sociobot license verification; the browser cannot unlock them by itself. Requests are capped at 64 KB and setup, login and public submission endpoints are rate-limited per connection.

The app loads no third-party runtime scripts, fonts or analytics. License verification calls the Sociobot API only when a pass is present. See `/privacy` and `/terms` in the running application.

## Deployment

The factory deploys the root `Dockerfile`; this repository does not manage DNS, billing registration or infrastructure. The Sociobot product is registered after handoff, so the code uses only the documented product slug and contains no product ID or provider secret.

## License

MIT — see [LICENSE](LICENSE).
