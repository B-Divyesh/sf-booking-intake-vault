# Authentication boundary

Manager access uses only the shared Sociobot Microsoft Entra External ID tenant:

- Authority: `https://sociobotcustomers.ciamlogin.com/35c6fe40-0ec0-46b6-98c6-213ad4de6650/`
- Public client ID: `25c704f4-465a-47af-80ab-2c489466b697`
- Claims checked by the server: RS256 signature, discovery issuer, JWKS key,
  client audience, tenant ID, expiry/not-before, and non-empty stable `oid`
- Browser cache: MSAL session storage; no local manager password exists

The first valid Entra identity claims a new single-team vault. The stored `oid`
must match for every later manager request. Other identities receive 403.
Workers do not sign in. They receive one narrow, random, expiring link whose
token is stored only as a SHA-256 hash.

`TEST_ENTRA_OID` and `X-Test-Oid` provide deterministic browser automation only.
The header is ignored by production binaries unless the exact environment
switch is explicitly present; deployed revisions do not set it.
