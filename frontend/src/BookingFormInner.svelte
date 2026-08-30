<script lang="ts">
  import { onMount } from 'svelte';
  import { formatDate } from './lib';
  type ApiRequest = <T>(url: string, options?: RequestInit) => Promise<T>;
  let { request, navigate } = $props<{ request: ApiRequest; navigate: (path: string) => void; state: object }>();
  type Field = { id: string; label: string; field_type: string; required: boolean; options: string[] };
  let form = $state<{ available?: boolean; error?: string; business_name: string; region: string; deletion_days: number; fields: Field[] } | null>(null);
  let values = $state<Record<string, string>>({});
  let error = $state(''); let loading = $state(true); let saving = $state(false); let complete = $state('');

  async function load() { loading = true; error = ''; try { const result = await request('/api/form/public') as typeof form; if (result?.available === false) { form = null; error = result.error || 'The form is unavailable.'; } else form = result; } catch (e) { error = e instanceof Error ? e.message : 'The form is unavailable.'; } finally { loading = false; } }
  async function submit(event: SubmitEvent) {
    event.preventDefault(); error = ''; saving = true;
    try { const result = await request('/api/bookings', { method: 'POST', body: JSON.stringify({ values, website: '' }) }) as { delete_at: string }; complete = result.delete_at; }
    catch (e) { error = e instanceof Error ? e.message : 'The request could not be sent.'; }
    finally { saving = false; }
  }
  onMount(load);
</script>

{#if loading}
  <section class="form-shell booking-route" aria-busy="true" aria-live="polite">
    <div class="form-intro"><p class="eyebrow">Booking request</p><h1>Tell the team<br />about the job.</h1><p>Loading the team’s questions…</p></div>
    <div class="paper-form booking-form-skeleton" aria-hidden="true">
      {#each Array(7) as _}<span></span>{/each}
    </div>
  </section>
{:else if error && !form}
  <section class="form-shell booking-route unavailable-booking" role="alert">
    <div class="form-intro"><p class="eyebrow">Booking request</p><h1>The booking desk is unavailable</h1><p>{error}</p></div>
    <div class="paper-form booking-error-panel"><h2>Try the connection again</h2><p>The booking questions could not be loaded. No details have been entered or sent.</p><button onclick={load}>Try again</button></div>
  </section>
{:else if complete}
  <section class="confirmation">
    <span class="confirmation-mark" aria-hidden="true">✓</span><p class="eyebrow">Request received</p><h1>Your details reached the manager.</h1>
    <p>The team can now prepare the right brief for the assigned worker. Your submission is scheduled for deletion on <strong>{formatDate(complete)}</strong>.</p>
    <div class="ticket-stub"><span>Private by default</span><strong>Only approved job details reach the worker</strong></div>
    <button class="secondary" onclick={() => navigate('/')}>Return to Private Intake</button>
  </section>
{:else if form}
  <section class="form-shell booking-route">
    <div class="form-intro"><p class="eyebrow">Hosted securely for {form.business_name}</p><h1>Tell the team<br />about the job.</h1><p>Fields marked “required” must be completed. Your manager decides what the assigned worker can see.</p></div>
    <form onsubmit={submit}>
      <div class="form-route" aria-hidden="true"><i></i><span>01</span><span>02</span><span>03</span></div>
      {#each form.fields as field}
        <div class="field-group">
          <label for={field.id}>{field.label}{#if field.required}<span class="required"> (required)</span>{/if}</label>
          {#if field.field_type === 'textarea'}
            <textarea id={field.id} bind:value={values[field.id]} required={field.required} maxlength="2000" rows="4"></textarea>
          {:else if field.field_type === 'select'}
            <select id={field.id} bind:value={values[field.id]} required={field.required}><option value="">Choose one</option>{#each field.options as option}<option value={option}>{option}</option>{/each}</select>
          {:else}
            <input id={field.id} type={field.field_type} bind:value={values[field.id]} required={field.required} maxlength="2000" />
          {/if}
        </div>
      {/each}
      <div class="honeypot" aria-hidden="true"><label for="website">Website</label><input id="website" tabindex="-1" autocomplete="off" /></div>
      {#if error}<p class="form-error" role="alert">{error}</p>{/if}
      <aside class="privacy-note"><span aria-hidden="true">◆</span><div><strong>Your details have two visibility levels.</strong><p>Retained for {form.deletion_days} days, then deleted. Only job and access details reach a worker; private context remains with the manager. Regional notice: {form.region}.</p></div></aside>
      <button type="submit" disabled={saving}>{saving ? 'Sending securely…' : 'Send booking request'} <span aria-hidden="true">→</span></button>
    </form>
  </section>
{/if}
