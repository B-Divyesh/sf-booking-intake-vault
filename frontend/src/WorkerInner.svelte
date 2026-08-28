<script lang="ts">
  import { onMount } from 'svelte';
  import { formatDate } from './lib';
  type ApiRequest = <T>(url: string, options?: RequestInit) => Promise<T>;
  let { token, request } = $props<{ token: string; request: ApiRequest }>();
  type ResponseItem = { field_id: string; label_snapshot: string; visibility_snapshot: string; value: string; sort_order: number };
  let brief = $state<{ available?: boolean; error?: string; created_at: string; delete_at: string; status: string; worker_name?: string; responses: ResponseItem[] } | null>(null);
  let loading = $state(true); let error = $state('');
  async function load() { loading = true; error = ''; try { const result = await request(`/api/worker/${encodeURIComponent(token)}`) as typeof brief; if (result?.available === false) { brief = null; error = result.error || 'This worker brief is unavailable.'; } else brief = result; } catch (e) { error = e instanceof Error ? e.message : 'This worker brief is unavailable.'; } finally { loading = false; } }
  onMount(load);
</script>

{#if loading}
  <section class="state-screen" aria-busy="true"><div class="route-loader" aria-hidden="true"><i></i><i></i><i></i></div><h1>Collecting the job brief</h1><p>Loading only the assigned details…</p></section>
{:else if error}
  <section class="state-screen compact" role="alert"><p class="eyebrow">Ticket expired</p><h1>This brief can’t be opened</h1><p>{error} Ask the manager for a new assignment link.</p><button onclick={load}>Try again</button></section>
{:else if brief}
  <article class="worker-ticket">
    <header><div><p class="eyebrow green">◎ Worker ticket</p><h1>Job brief{brief.worker_name ? ` for ${brief.worker_name}` : ''}</h1></div><span class="status">{brief.status === 'complete' ? 'Complete' : 'Ready for the job'}</span></header>
    <p class="worker-intro">This brief contains operational facts approved for the assigned worker. Client contact and commercial notes are not included.</p>
    <dl class="response-list worker-list">
      {#each brief.responses as item}<div><dt>{item.label_snapshot}</dt><dd>{item.value}</dd></div>{/each}
    </dl>
    {#if brief.responses.length === 0}<div class="empty-state"><h2>No worker details are available</h2><p>Ask the manager to review the field visibility before attending.</p></div>{/if}
    <footer><span>Opened {formatDate(new Date().toISOString(), true)}</span><span>Link expires or data deletes by {formatDate(brief.delete_at, true)}</span></footer>
  </article>
{/if}
