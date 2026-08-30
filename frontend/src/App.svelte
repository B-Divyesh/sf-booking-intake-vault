<script lang="ts">
  import { onMount } from 'svelte';
  import { BILLING_BASE, PRODUCT_SLUG, LICENSE_KEY, VERDICT_KEY, cachedVerdict, captureLicense, formatDate } from './lib';
  import BookingFormInner from './BookingFormInner.svelte';
  import AdminInner from './AdminInner.svelte';
  import WorkerInner from './WorkerInner.svelte';

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

  function navigate(next: string) {
    history.pushState({}, '', next);
    path = next;
    error = ''; notice = '';
    window.scrollTo({ top: 0, behavior: matchMedia('(prefers-reduced-motion: reduce)').matches ? 'auto' : 'smooth' });
  }

  async function request<T>(url: string, options: RequestInit = {}): Promise<T> {
    // Let routes render their explicit offline recovery state without first
    // issuing a disconnected API request that Chromium reports as an error.
    if (!navigator.onLine) throw new Error('You’re offline. Reconnect and try again.');
    const headers = new Headers(options.headers);
    headers.set('content-type', 'application/json');
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
    addEventListener('popstate', () => { path = location.pathname; });
    addEventListener('online', () => { online = true; });
    addEventListener('offline', () => { online = false; });
    await Promise.all([loadSession(), verifyLicense()]);
    loading = false;
  });
</script>

<svelte:head>
  <title>{path === '/book' ? 'Book a service' : path.startsWith('/worker/') ? 'Worker job brief' : path === '/admin' ? 'Manager vault' : path === '/privacy' ? 'Privacy notice' : path === '/terms' ? 'Terms of use' : 'Private Intake — least-privilege booking forms'}</title>
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
    <a href="/book" onclick={(event) => { event.preventDefault(); navigate('/book'); }}>Booking form</a>
    <a href="/admin" class="nav-ticket" onclick={(event) => { event.preventDefault(); navigate('/admin'); }}>Manager vault</a>
  </nav>
</header>

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
  {:else if path === '/book'}
    <BookingFormInner {request} {navigate} state={{}} />
  {:else if path === '/admin'}
    <AdminInner {request} bind:session {loadSession} {licenseValid} {licenseReason} bind:licenseInput {restoreLicense} />
  {:else if path.startsWith('/worker/')}
    <WorkerInner token={path.slice('/worker/'.length)} {request} />
  {:else if path === '/privacy'}
    <Privacy />
  {:else if path === '/terms'}
    <Terms />
  {:else}
    <section class="state-screen"><p class="eyebrow">Route closed</p><h1>That stop isn’t on this line</h1><p>The page may have moved or the worker link may be incomplete.</p><button onclick={() => navigate('/')}>Return home</button></section>
  {/if}

  {#if error}<div class="toast error" role="alert">{error}</div>{/if}
  {#if notice}<div class="toast" role="status">{notice}</div>{/if}
</main>

<footer>
  <div><span class="footer-mark" aria-hidden="true">PI</span><p><strong>Private Intake</strong><br />The right details. The right hands.</p></div>
  <nav aria-label="Legal">
    <a href="/privacy" onclick={(e) => { e.preventDefault(); navigate('/privacy'); }}>Privacy</a>
    <a href="/terms" onclick={(e) => { e.preventDefault(); navigate('/terms'); }}>Terms</a>
  </nav>
  <p class="disclosure">Hero artwork is original AI-generated imagery, created for this product.</p>
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
    <div><strong>Details travel on separate routes.</strong><p>Your information is retained for {days} days, then deleted. The assigned worker receives only job and access details. Contact and billing context stay with the manager. Regional notice: {region}.</p></div>
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
      {@render headingEyebrow('Least-privilege booking for field teams')}
      <h1>Client details,<br /><em>routed with care.</em></h1>
      <p class="lede">Collect the whole story once. Send each worker a clean job brief with only what they need—and keep private context in the manager’s vault.</p>
      <div class="action-row">
        <a class="button" href="/admin" onclick={(e) => { e.preventDefault(); navigate('/admin'); }}>Set up your vault <span aria-hidden="true">→</span></a>
        <a class="text-link" href="#how">See the privacy route <span aria-hidden="true">↓</span></a>
      </div>
      <ul class="trust-list" aria-label="Product assurances"><li>Server-enforced roles</li><li>Automatic deletion</li><li>No trackers</li></ul>
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
    <div class="section-heading">{@render headingEyebrow('One intake · two destinations')}<h2>Privacy is a route,<br />not a promise.</h2></div>
    <ol class="route-steps">
      {@render feature('01', 'Collect once', 'A calm hosted form gathers contact, site, schedule and private commercial context.')}
      {@render feature('02', 'Mark every field', 'Each question is explicitly tagged Worker sees or Manager only before a client submits.')}
      {@render feature('03', 'Share a clean brief', 'An expiring link contains only worker-safe facts. Private answers never enter that response.')}
    </ol>
  </section>

  <section class="split-proof">
    <div><p class="eyebrow coral">◆ Manager vault</p><h2>Client name · phone · billing</h2><p>Full context remains behind your passphrase.</p></div>
    <div><p class="eyebrow green">◎ Worker ticket</p><h2>Address · arrival · job notes</h2><p>A clear operational brief, without the overshare.</p></div>
  </section>

  <section class="cta-band">
    <div><p class="eyebrow">Next departure</p><h2>Open a safer intake route today.</h2></div>
    <a class="button" href="/admin" onclick={(e) => { e.preventDefault(); navigate('/admin'); }}>Build the form <span aria-hidden="true">→</span></a>
  </section>
{/snippet}

{#snippet Privacy()}
  <article class="legal-page">
    {@render headingEyebrow('Privacy notice · effective 28 August 2026')}
    <h1>Short retention.<br />Narrow access.</h1>
    <p class="lede">Private Intake is designed to minimize the client information a field worker can access. The team operating a vault is the controller of information submitted through its form; Private Intake processes it to provide the service.</p>
    {@render decoRule()}
    <h2>What is stored</h2><p>We store answers entered on the booking form, submission and deletion timestamps, job status, assigned worker name, and a hashed access credential. Manager passphrases and worker-link tokens are stored as one-way hashes.</p>
    <h2>Who can see it</h2><p>Managers can view and export all answers. A worker with a live assignment link receives only answers that were marked “Worker sees” when submitted. This filter is enforced by the server.</p>
    <h2>Retention and deletion</h2><p>The manager selects a retention window of 1–90 days. Expired submissions and their worker links are deleted automatically. Managers can export or delete a booking earlier from the vault.</p>
    <h2>Infrastructure and billing</h2><p>No advertising analytics, tracking scripts, third-party fonts, or client identifiers in application logs are used. If you buy a Route pass, Sociobot/Dodo is the merchant of record and processes payment; this app stores only your license token on this device.</p>
    <h2>Your choices</h2><p>Contact the field-service team named on the booking form to request access, correction, export or early deletion. For service privacy questions, email <a href="mailto:privacy@sociobot.in">privacy@sociobot.in</a>.</p>
  </article>
{/snippet}

{#snippet Terms()}
  <article class="legal-page">
    {@render headingEyebrow('Terms of use · effective 28 August 2026')}
    <h1>A clear route<br />for responsible use.</h1>
    <p class="lede">Private Intake is a booking-intake and least-privilege job-brief service. It is not a calendar, emergency dispatch system, payment processor or customer record system.</p>
    {@render decoRule()}
    <h2>Your responsibilities</h2><p>You must have a lawful reason to collect submitted information, ask only for what your service needs, configure worker visibility carefully, keep assignment links secure, and respond to data-rights requests.</p>
    <h2>Availability and safety</h2><p>The service is provided as-is. Do not use it as the sole channel for emergencies, medical instructions or safety-critical dispatch. Review the redaction preview before sharing every worker link.</p>
    <h2>Route pass</h2><p>The optional Route pass is a one-time US$29 purchase that unlocks up to 12 custom questions and longer worker-link choices for this hosted vault. The server verifies the license before saving either upgrade. The hosted checkout shows local taxes before payment. Sociobot/Dodo is the merchant of record and handles refund requests; a refund revokes the license automatically. Core export, deletion and accessibility features remain free.</p>
    <h2>Acceptable use</h2><p>Do not submit illegal content, secrets unrelated to a job, payment card data, health records, or credentials. Do not probe another team’s vault or share a worker link outside the assigned job.</p>
    <h2>Contact</h2><p>Questions about these terms can be sent to <a href="mailto:support@sociobot.in">support@sociobot.in</a>.</p>
  </article>
{/snippet}
