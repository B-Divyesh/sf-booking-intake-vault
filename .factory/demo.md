# Private Intake demo

- URL: `https://booking-intake-vault.sociobot.in/demo`
- Local URL: `http://127.0.0.1:8080/demo`
- Entry point: the first-screen **Try it with sample data** link.

The demo creates a random, in-memory workspace with a 24-hour expiry. It shows
one realistic booking for Nadia Patel, the manager's complete eight-answer
record, and Morgan Lee's five-answer worker brief. The client caches that sample
only under `sessionStorage` keys prefixed `demo:private-intake:` so an offline
reload works. Demo routes never query or mutate the production SQLite tables.

**Reset demo** invalidates the current in-memory workspace and creates a fresh
one with a new random ID. **Start for real** clears the demo session keys and
opens the public booking form. No account, license, or external network request
is needed for the demo.
