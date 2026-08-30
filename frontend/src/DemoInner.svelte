<script lang="ts">
  import { onMount } from 'svelte';
  import { formatDate } from './lib';

  type ResponseItem = { field_id: string; label_snapshot: string; visibility_snapshot: 'worker' | 'admin'; value: string; sort_order: number };
  type DemoWorkspace = {
    id: string; created_at: string; expires_at: string; delete_at: string;
    worker_name: string; status: string;
    manager_responses: ResponseItem[]; worker_responses: ResponseItem[];
  };
  type ApiRequest = <T>(url: string, options?: RequestInit) => Promise<T>;
  let { request, navigate } = $props<{ request: ApiRequest; navigate: (path: string) => void }>();
  let workspace = $state<DemoWorkspace | null>(null);
  let loading = $state(true);
  let error = $state('');
  const idKey = 'demo:private-intake:id';
  const dataKey = 'demo:private-intake:data';

  function cache(data: DemoWorkspace) {
    workspace = data;
    sessionStorage.setItem(idKey, data.id);
    sessionStorage.setItem(dataKey, JSON.stringify(data));
  }

  function cached(): DemoWorkspace | null {
    try { return JSON.parse(sessionStorage.getItem(dataKey) || 'null') as DemoWorkspace | null; }
    catch { return null; }
  }

  async function load() {
    loading = true; error = '';
    const id = sessionStorage.getItem(idKey);
    try {
      const data = (id
        ? await request(`/api/demo/workspaces/${encodeURIComponent(id)}`)
        : await request('/api/demo/workspaces', { method: 'POST', body: '{}' })) as DemoWorkspace;
      cache(data);
    } catch (reason) {
      const saved = cached();
      if (saved) workspace = saved;
      else error = reason instanceof Error ? reason.message : 'The sample workspace could not be opened.';
    } finally { loading = false; }
  }

  async function reset() {
    if (!workspace) return;
    error = '';
    try {
      cache(await request(`/api/demo/workspaces/${encodeURIComponent(workspace.id)}/reset`, { method: 'POST', body: '{}' }) as DemoWorkspace);
    } catch (reason) {
      error = reason instanceof Error ? reason.message : 'The sample could not be reset.';
    }
  }

  function startForReal() {
    sessionStorage.removeItem(idKey);
    sessionStorage.removeItem(dataKey);
    navigate('/book');
  }

  onMount(load);
</script>

<div class="demo-banner" role="status">
  <strong>Demo — sample data, nothing is saved</strong>
  <span>
    <button class="quiet" onclick={reset} disabled={!workspace}>Reset demo</button>
    <button class="quiet" onclick={startForReal}>Start for real</button>
  </span>
</div>

{#if loading}
  <section class="state-screen compact" aria-busy="true"><div class="route-loader" aria-hidden="true"><i></i><i></i><i></i></div><h1>Loading the sample booking</h1><p>Preparing a separate workspace with realistic data.</p></section>
{:else if error && !workspace}
  <section class="state-screen compact" role="alert"><p class="eyebrow">Demo unavailable</p><h1>The sample could not be opened</h1><p>{error}</p><button onclick={load}>Try again</button></section>
{:else if workspace}
  <section class="demo-shell">
    <header class="demo-heading">
      <div><p class="eyebrow">Sample booking · assigned</p><h1>See a private booking split safely</h1><p>The manager keeps all eight answers. Morgan receives only five job details.</p></div>
      <div class="demo-actions"><a class="button-like secondary" href={`/api/demo/workspaces/${workspace.id}/export.csv`} download>Export sample CSV</a><button onclick={reset}>Reset sample</button></div>
    </header>
    {#if error}<p class="inline-alert error" role="alert">{error}</p>{/if}
    <div class="demo-compare">
      <article class="booking-full" data-testid="demo-manager-record">
        <p class="eyebrow coral">◆ Manager’s complete record</p>
        <h2>Nadia Patel’s booking</h2>
        <p>Received {formatDate(workspace.created_at, true)} · deletes {formatDate(workspace.delete_at)}</p>
        <dl class="response-list">
          {#each workspace.manager_responses as item}
            <div><dt>{item.label_snapshot} <span class="badge {item.visibility_snapshot === 'worker' ? 'worker-badge' : 'admin-badge'}">{item.visibility_snapshot === 'worker' ? '◎ Worker sees' : '◆ Manager only'}</span></dt><dd>{item.value}</dd></div>
          {/each}
        </dl>
      </article>
      <article class="preview-ticket" data-testid="demo-worker-brief">
        <p class="eyebrow green">◎ Worker ticket</p>
        <h2>Job brief for {workspace.worker_name}</h2>
        <p>This server-built view omits the client’s name, phone, and billing notes.</p>
        <dl class="response-list">
          {#each workspace.worker_responses as item}<div><dt>{item.label_snapshot}</dt><dd>{item.value}</dd></div>{/each}
        </dl>
        <p class="demo-expiry"><strong>Expiring link</strong><br />This sample workspace closes {formatDate(workspace.expires_at, true)}.</p>
      </article>
    </div>
  </section>
{/if}
