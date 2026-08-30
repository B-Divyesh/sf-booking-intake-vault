<script lang="ts">
  import { onMount, tick } from 'svelte';
  import { BILLING_BASE, PRODUCT_SLUG, LICENSE_KEY, VERDICT_KEY, cachedVerdict, captureLicense, formatDate } from './lib';
  import { accessToken, finishSignIn, signIn, signOut, testOwnerOid } from './auth';
  import BookingFormInner from './BookingFormInner.svelte';
  import AdminInner from './AdminInner.svelte';
  import WorkerInner from './WorkerInner.svelte';
  import DemoInner from './DemoInner.svelte';

  type Field = { id?: string; label: string; field_type: string; required: boolean; visibility: 'worker' | 'admin'; options: string[] };
  type ResponseItem = { field_id: string; label_snapshot: string; visibility_snapshot: 'worker' | 'admin'; value: string; sort_order: number };
  type Booking = { id: string; created_at: string; delete_at: string; status: string; worker_name?: string; summary?: string; responses?: ResponseItem[] };
  type Workspace = { business_name: string; timezone: string; region: string; deletion_days: number };

  let path = window.location.pathname;
  let online = navigator.onLine;
  let loading = true;
  let error = '';
  let notice = '';
  let session = { configured: false, authenticated: false, workspace: null as Workspace | null };
  let licenseValid = false;
  let licenseReason = '';
  let licenseInput = '';
  let buildId = 'development';
  let routeAnnouncement = '';
  let routeFocusDeadline = 0;
  let focusedRouteHeading: HTMLElement | null = null;

  function titleFor(currentPath: string) {
    if (currentPath === '/book') return 'Book a service — Private Intake';
    if (currentPath === '/demo') return 'Demo — Private Intake';
    if (currentPath === '/admin' || currentPath === '/auth/callback') return 'Manager vault — Private Intake';
    if (currentPath === '/privacy') return 'Privacy — Private Intake';
    if (currentPath === '/terms') return 'Terms — Private Intake';
    if (currentPath.startsWith('/worker/')) return 'Worker job brief — Private Intake';
    if (currentPath !== '/') return 'Page not found — Private Intake';
    return 'Private Intake — Share only needed job details';
  }

  function focusCurrentHeading() {
    if (Date.now() > routeFocusDeadline) return;
    const heading = document.querySelector<HTMLElement>('main h1');
    if (heading && heading !== focusedRouteHeading) {
      focusedRouteHeading = heading;
      heading.tabIndex = -1;
      heading.focus({ preventScroll: true });
      routeAnnouncement = heading.textContent?.trim() || titleFor(path);
    }
  }

  async function finishRouteChange() {
    routeFocusDeadline = Date.now() + 5000;
    focusedRouteHeading = null;
    await tick();
    focusCurrentHeading();
  }

  function navigate(next: string) {
    history.pushState({}, '', next);
    path = next;
    error = ''; notice = '';
    window.scrollTo({ top: 0, behavior: matchMedia('(prefers-reduced-motion: reduce)').matches ? 'auto' : 'smooth' });
    void finishRouteChange();
  }

  async function request<T>(url: string, options: RequestInit = {}): Promise<T> {
    // Let routes render their explicit offline recovery state without first
    // issuing a disconnected API request that Chromium reports as an error.
    if (!navigator.onLine) throw new Error('You’re offline. Reconnect and try again.');
    const headers = new Headers(options.headers);
    headers.set('content-type', 'application/json');
    const testOid = testOwnerOid();
    if (testOid) headers.set('x-test-oid', testOid);
    if (testOid || path === '/admin' || path === '/auth/callback') {
      const identityToken = await accessToken();
      if (identityToken) headers.set('authorization', `Bearer ${identityToken}`);
    }
    const license = licenseValid ? localStorage.getItem(LICENSE_KEY) : null;
    if (license) headers.set('x-route-license', license);
    const response = await fetch(url, { ...options, credentials: 'same-origin', headers });
    const contentType = response.headers.get('content-type') || '';
    const body = contentType.includes('json') ? await response.json() : null;
    if (!response.ok) throw new Error(body?.error || 'The request could not be completed.');
    return body as T;
  }

  async function loadSession() {
    try { session = await request('/api/session'); }
    catch (reason) { error = reason instanceof Error ? reason.message : 'Could not reach the vault.'; }
  }

  async function verifyLicense(force = false) {
    const token = localStorage.getItem(LICENSE_KEY);
    if (!token) return;
    const cached = cachedVerdict(localStorage);
    if (cached && !force) { licenseValid = cached.valid; licenseReason = cached.reason; return; }
    try {
      const response = await fetch(`${BILLING_BASE}/products/${PRODUCT_SLUG}/verify?license=${encodeURIComponent(token)}`);
      const result = await response.json();
      const verdict = { ...result, checkedAt: Date.now() };
      localStorage.setItem(VERDICT_KEY, JSON.stringify(verdict));
      licenseValid = Boolean(result.valid); licenseReason = result.reason;
    } catch {
      const old = (() => { try { return JSON.parse(localStorage.getItem(VERDICT_KEY) || 'null'); } catch { return null; } })();
      licenseValid = Boolean(old?.valid);
    }
  }

  function restoreLicense() {
    const value = licenseInput.trim();
    if (!value) return;
    localStorage.setItem(LICENSE_KEY, value);
    localStorage.removeItem(VERDICT_KEY);
    licenseInput = '';
    verifyLicense(true);
  }

  onMount(async () => {
    const url = new URL(window.location.href);
    if (captureLicense(url, localStorage)) {
      url.searchParams.delete('license'); history.replaceState({}, '', url.pathname + url.search + url.hash); notice = 'License received. Verifying your Route pass…';
    }
    if (path === '/auth/callback') {
      try {
        if (await finishSignIn()) {
          history.replaceState({}, '', '/admin');
          path = '/admin';
        }
      } catch { error = 'Sociobot sign-in could not be completed. Try again.'; }
    }
    addEventListener('popstate', () => { path = location.pathname; void finishRouteChange(); });
    addEventListener('online', () => { online = true; });
    addEventListener('offline', () => { online = false; });
    const routeObserver = new MutationObserver(() => requestAnimationFrame(focusCurrentHeading));
    const main = document.querySelector('main');
    if (main) routeObserver.observe(main, { childList: true, subtree: true });
    const health = fetch('/health').then((response) => response.json()).then((value) => { buildId = String(value.build || 'development'); }).catch(() => {});
    const identity = path === '/admin' || path === '/auth/callback' ? loadSession() : Promise.resolve();
    await Promise.all([identity, verifyLicense(), health]);
    loading = false;
  });
</script>

<svelte:head>
  <title>{titleFor(path)}</title>
  <link rel="canonical" href={`https://booking-intake-vault.sociobot.in${path === '/auth/callback' ? '/admin' : path}`} />
  <meta property="og:title" content={titleFor(path)} />
  <meta property="og:description" content="Collect client details once and send assigned workers only the job facts they need." />
  <meta property="og:type" content="website" />
  <meta property="og:url" content={`https://booking-intake-vault.sociobot.in${path === '/auth/callback' ? '/admin' : path}`} />
  <meta property="og:image" content="https://booking-intake-vault.sociobot.in/assets/private-routes-social.webp" />
  <meta name="twitter:card" content="summary_large_image" />
  <meta name="twitter:title" content={titleFor(path)} />
  <meta name="twitter:description" content="Collect client details once and send assigned workers only the job facts they need." />
  <meta name="twitter:image" content="https://booking-intake-vault.sociobot.in/assets/private-routes-social.webp" />
  {#if path === '/'}
    <link rel="preload" as="image" href="/assets/private-routes-1200.webp" imagesrcset="/assets/private-routes-720.webp 720w, /assets/private-routes-1200.webp 1200w" imagesizes="(max-width: 700px) calc(100vw - 28px), min(56vw, 700px)" fetchpriority="high" />
  {/if}
</svelte:head>

<header class="site-header">
  <a href="/" class="brand" onclick={(event) => { event.preventDefault(); navigate('/'); }} aria-label="Private Intake home">
    <span class="brand-mark" aria-hidden="true"><i></i><i></i><i></i></span>
    <span>Private Intake</span>
  </a>
  <nav aria-label="Primary navigation">
    <a href="/demo" onclick={(event) => { event.preventDefault(); navigate('/demo'); }}>Demo</a>
    <a href="/book" onclick={(event) => { event.preventDefault(); navigate('/book'); }}>Booking form</a>
    <a href="/admin" class="nav-ticket" onclick={(event) => { event.preventDefault(); navigate('/admin'); }}>Manager vault</a>
  </nav>
</header>

<div class="route-announcement" aria-live="polite" aria-atomic="true">{routeAnnouncement}</div>

{#if !online}
  <div class="offline" role="status"><span aria-hidden="true">◇</span> You’re offline. Existing details stay visible; reconnect before saving.</div>
{/if}

<main id="main">
  {#if loading && path === '/admin'}
    <section class="state-screen" aria-busy="true" aria-live="polite">
      <div class="route-loader" aria-hidden="true"><i></i><i></i><i></i></div>
      <h1>Opening the route</h1>
      <p>Preparing the private intake vault…</p>
    </section>
  {:else if path === '/'}
    {@render Landing(navigate)}
  {:else if path === '/demo'}
    <DemoInner {request} {navigate} />
  {:else if path === '/book'}
    <BookingFormInner {request} {navigate} state={{}} />
  {:else if path === '/admin' || path === '/auth/callback'}
    <AdminInner {request} bind:session {loadSession} {licenseValid} {licenseReason} bind:licenseInput {restoreLicense} {signIn} {signOut} />
  {:else if path.startsWith('/worker/')}
    <WorkerInner token={path.slice('/worker/'.length)} {request} />
  {:else if path === '/privacy'}
    <Privacy />
  {:else if path === '/terms'}
    <Terms />
  {:else}
    <section class="state-screen"><p class="eyebrow">Page not found</p><h1>This page does not exist</h1><p>Check the address or return to the Private Intake home page.</p><button onclick={() => navigate('/')}>Return home</button></section>
  {/if}

  {#if error}<div class="toast error" role="alert">{error}</div>{/if}
  {#if notice}<div class="toast" role="status">{notice}</div>{/if}
</main>

<footer>
  <div><span class="footer-mark" aria-hidden="true">PI</span><p><strong>Private Intake</strong><br />Share only the job details a worker needs.</p></div>
  <nav aria-label="Legal">
    <a href="/privacy" onclick={(e) => { e.preventDefault(); navigate('/privacy'); }}>Privacy</a>
    <a href="/terms" onclick={(e) => { e.preventDefault(); navigate('/terms'); }}>Terms</a>
  </nav>
  <p class="disclosure">Built by Param Factory · Build {buildId.slice(0, 12)}<br />Original AI-generated hero artwork.</p>
</footer>

{#snippet routeIcon(kind: 'worker' | 'admin')}
  <span class:worker-icon={kind === 'worker'} class:admin-icon={kind === 'admin'} aria-hidden="true">{kind === 'worker' ? '◎' : '◆'}</span>
{/snippet}

{#snippet visibilityBadge(visibility: 'worker' | 'admin')}
  <span class:worker-badge={visibility === 'worker'} class:admin-badge={visibility === 'admin'} class="badge">
    {@render routeIcon(visibility)} {visibility === 'worker' ? 'Worker sees' : 'Manager only'}
  </span>
{/snippet}

{#snippet responseList(responses: ResponseItem[], showVisibility = false)}
  <dl class="response-list">
    {#each responses as item}
      <div>
        <dt>{item.label_snapshot} {#if showVisibility}{@render visibilityBadge(item.visibility_snapshot)}{/if}</dt>
        <dd>{item.value}</dd>
      </div>
    {/each}
  </dl>
{/snippet}

{#snippet statusChip(status: string)}
  <span class="status status-{status}">{status === 'new' ? 'New request' : status === 'assigned' ? 'Worker assigned' : 'Complete'}</span>
{/snippet}

{#snippet fieldInput(field: Field, values: Record<string, string>)}
  {#if field.field_type === 'textarea'}
    <textarea id={field.id} bind:value={values[field.id || '']} required={field.required} maxlength="2000" rows="4"></textarea>
  {:else if field.field_type === 'select'}
    <select id={field.id} bind:value={values[field.id || '']} required={field.required}>
      <option value="">Choose one</option>
      {#each field.options as option}<option value={option}>{option}</option>{/each}
    </select>
  {:else}
    <input id={field.id} type={field.field_type} bind:value={values[field.id || '']} required={field.required} maxlength="2000" />
  {/if}
{/snippet}

{#snippet emptyStation(title: string, copy: string)}
  <div class="empty-state">
    <span class="empty-symbol" aria-hidden="true">◇</span>
    <h2>{title}</h2>
    <p>{copy}</p>
  </div>
{/snippet}

{#snippet decoRule()}
  <div class="deco-rule" aria-hidden="true"><i></i><span>◆</span><i></i></div>
{/snippet}

{#snippet errorPanel(message: string, retry: () => void)}
  <section class="state-screen compact" role="alert"><p class="eyebrow">Route interrupted</p><h1>We couldn’t open this stop</h1><p>{message}</p><button onclick={retry}>Try again</button></section>
{/snippet}

{#snippet publicPrivacy(region: string, days: number)}
  <aside class="privacy-note">
    <span aria-hidden="true">◆</span>
    <div><strong>Your details have two visibility levels.</strong><p>Your information is retained for {days} days, then deleted. The assigned worker receives only job and access details. Contact and billing context stay with the manager. Regional notice: {region}.</p></div>
  </aside>
{/snippet}

{#snippet feature(ordinal: string, title: string, copy: string)}
  <li><span>{ordinal}</span><div><h3>{title}</h3><p>{copy}</p></div></li>
{/snippet}

{#snippet headingEyebrow(text: string)}<p class="eyebrow"><span aria-hidden="true">◆</span> {text}</p>{/snippet}

{#snippet pageActions()}{/snippet}

{#snippet Landing(navigate: (path: string) => void)}
  <section class="hero">
    <div class="hero-copy">
      {@render headingEyebrow('Private booking for field-service teams')}
      <h1>Collect once. <em>Share only job details.</em></h1>
      <p class="lede">For field-service teams that keep client context private when they assign work.</p>
      <div class="action-row">
        <a class="button" href="/demo" onclick={(e) => { e.preventDefault(); navigate('/demo'); }}>Try it with sample data <span aria-hidden="true">→</span></a>
        <span class="action-explainer">See one booking split into manager and worker views.</span>
        <a class="text-link" href="/book" onclick={(e) => { e.preventDefault(); navigate('/book'); }}>Open the booking form</a>
      </div>
      <ul class="trust-list" aria-label="Product facts"><li>Core booking tools are free</li><li>Works offline after your first visit</li><li>No trackers or third-party scripts</li></ul>
    </div>
    <figure class="hero-poster">
      <picture>
        <source media="(max-width: 700px)" srcset="/assets/private-routes-720.webp" />
        <img src="/assets/private-routes-1200.webp" width="1200" height="800" alt="Art-deco station illustration where one lit route separates toward a field-worker platform and a sealed private vault" fetchpriority="high" decoding="async" />
      </picture>
      <figcaption><span>Route 01</span> Intake separates before assignment</figcaption>
    </figure>
  </section>

  <section class="route-story" id="how">
    <div class="section-heading">{@render headingEyebrow('How it works')}<h2>Send each person only what they need</h2></div>
    <ol class="route-steps">
      {@render feature('01', 'Collect the booking', 'The hosted form gathers contact, site, schedule, and private account details.')}
      {@render feature('02', 'Choose who sees each answer', 'Every question says “Worker sees” or “Manager only” before a client submits.')}
      {@render feature('03', 'Send an expiring brief', 'The server builds a worker link from permitted job facts only.')}
    </ol>
  </section>

  <section class="split-proof">
    <div><p class="eyebrow coral">◆ Manager vault</p><h2>Client name · phone · billing</h2><p>Full context stays behind Sociobot sign-in.</p></div>
    <div><p class="eyebrow green">◎ Worker ticket</p><h2>Address · arrival · job notes</h2><p>A worker brief excludes contact and billing details.</p></div>
  </section>

  <section class="route-story limits-section">
    <div class="section-heading">{@render headingEyebrow('Scope and privacy')}<h2>What this service does not do</h2></div>
    <div><p>Private Intake does not schedule calendars, collect payments, dispatch emergencies, or replace a customer record system.</p><p>Managers choose each field’s visibility. The server filters every worker brief and removes each booking on its deletion date.</p></div>
  </section>

  <section class="cta-band">
    <div><p class="eyebrow">Optional Route pass</p><h2>Keep core tools free or add more fields</h2><p>Core booking, export, deletion, and accessibility are free. Pay US$29 once for 12 questions and longer worker links.</p></div>
    <a class="button" href="/admin" onclick={(e) => { e.preventDefault(); navigate('/admin'); }}>Open manager tools <span aria-hidden="true">→</span></a>
  </section>
{/snippet}

{#snippet Privacy()}
  <article class="legal-page">
    {@render headingEyebrow('Privacy notice · effective 30 August 2026')}
    <h1>How Private Intake handles data</h1>
    <p class="lede">Private Intake is designed to minimize the client information a field worker can access. The team operating a vault is the controller of information submitted through its form; Private Intake processes it to provide the service.</p>
    {@render decoRule()}
    <h2>What is stored</h2><p>We store booking answers, submission and deletion times, job status, assigned worker name, the manager’s Entra account ID, and hashed worker-link tokens.</p>
    <h2>Who can see it</h2><p>Managers can view and export all answers. A worker with a live assignment link receives only answers that were marked “Worker sees” when submitted. This filter is enforced by the server.</p>
    <h2>Retention and deletion</h2><p>The manager selects a retention window of 1–90 days. Expired submissions and their worker links are deleted automatically. Managers can export or delete a booking earlier from the vault.</p>
    <h2>Infrastructure and billing</h2><p>No advertising analytics, tracking scripts, third-party fonts, or client identifiers in application logs are used. If you buy a Route pass, Sociobot/Dodo is the merchant of record and processes payment; this app stores only your license token on this device.</p>
    <h2>Your choices</h2><p>Contact the field-service team named on the booking form to request access, correction, export or early deletion. For service privacy questions, email <a href="mailto:privacy@sociobot.in">privacy@sociobot.in</a>.</p>
  </article>
{/snippet}

{#snippet Terms()}
  <article class="legal-page">
    {@render headingEyebrow('Terms of use · effective 30 August 2026')}
    <h1>Terms for using Private Intake</h1>
    <p class="lede">Private Intake is a booking-intake and least-privilege job-brief service. It is not a calendar, emergency dispatch system, payment processor or customer record system.</p>
    {@render decoRule()}
    <h2>Your responsibilities</h2><p>You must have a lawful reason to collect submitted information, ask only for what your service needs, configure worker visibility carefully, keep assignment links secure, and respond to data-rights requests.</p>
    <h2>Availability and safety</h2><p>The service is provided as-is. Do not use it as the sole channel for emergencies, medical instructions or safety-critical dispatch. Review the redaction preview before sharing every worker link.</p>
    <h2>Route pass</h2><p>The optional Route pass is a one-time US$29 purchase that unlocks up to 12 custom questions and longer worker-link choices for this hosted vault. The server verifies the license before saving either upgrade. The hosted checkout shows local taxes before payment. Sociobot/Dodo is the merchant of record and handles refund requests; a refund revokes the license automatically. Core export, deletion and accessibility features remain free.</p>
    <h2>Acceptable use</h2><p>Do not submit illegal content, secrets unrelated to a job, payment card data, health records, or credentials. Do not probe another team’s vault or share a worker link outside the assigned job.</p>
    <h2>Contact</h2><p>Questions about these terms can be sent to <a href="mailto:support@sociobot.in">support@sociobot.in</a>.</p>
  </article>
{/snippet}
