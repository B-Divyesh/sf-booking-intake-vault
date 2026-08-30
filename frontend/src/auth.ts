import type { AccountInfo, PublicClientApplication } from '@azure/msal-browser';

const tenantId = import.meta.env.VITE_ENTRA_TENANT_ID ?? '35c6fe40-0ec0-46b6-98c6-213ad4de6650';
const subdomain = import.meta.env.VITE_ENTRA_TENANT_SUBDOMAIN ?? 'sociobotcustomers';
const clientId = import.meta.env.VITE_ENTRA_CLIENT_ID ?? '25c704f4-465a-47af-80ab-2c489466b697';
const scopes = ['openid', 'profile', 'email'];
const testIdentityKey = 'piv:test-owner-oid';

export const ENTRA_AUTHORITY = `https://${subdomain}.ciamlogin.com/${tenantId}/`;

let clientPromise: Promise<PublicClientApplication> | null = null;

async function identityClient(): Promise<PublicClientApplication> {
  clientPromise ??= import('@azure/msal-browser').then(({ PublicClientApplication }) => new PublicClientApplication({
    auth: {
      clientId,
      authority: ENTRA_AUTHORITY,
      redirectUri: `${window.location.origin}/auth/callback`,
      knownAuthorities: [`${subdomain}.ciamlogin.com`],
    },
    cache: { cacheLocation: 'sessionStorage' },
  }));
  return clientPromise;
}

let initialized = false;

async function account(): Promise<AccountInfo | null> {
  const client = await identityClient();
  if (!initialized) {
    await client.initialize();
    const redirect = await client.handleRedirectPromise();
    if (redirect?.account) client.setActiveAccount(redirect.account);
    initialized = true;
  }
  return client.getActiveAccount() ?? client.getAllAccounts()[0] ?? null;
}

export function testOwnerOid(): string | null {
  return sessionStorage.getItem(testIdentityKey);
}

export async function accessToken(): Promise<string | null> {
  if (testOwnerOid()) return 'local-test-identity';
  const client = await identityClient();
  const current = await account();
  if (!current) return null;
  try {
    const result = await client.acquireTokenSilent({ account: current, scopes });
    return result.idToken || result.accessToken || null;
  } catch {
    return null;
  }
}

export async function finishSignIn(): Promise<boolean> {
  return Boolean(await account());
}

export async function signIn(): Promise<void> {
  const client = await identityClient();
  await account();
  await client.loginRedirect({ scopes });
}

export async function signOut(): Promise<void> {
  if (testOwnerOid()) {
    sessionStorage.removeItem(testIdentityKey);
    return;
  }
  const client = await identityClient();
  const current = await account();
  if (current) {
    await client.logoutRedirect({ account: current, postLogoutRedirectUri: window.location.origin });
  }
}
