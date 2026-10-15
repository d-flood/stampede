<script lang="ts">
	let { screen }: { screen: 'working' | 'waiting' } = $props();
</script>

{#if screen === 'working'}
	<pre><span class="accent">*</span> <b>Claude Code</b> <span class="dim">v2.4 · ~/repos/oikonomos</span>

<span class="dim">&gt;</span> Reattach holders on startup and make sure the
  screen after a restart equals the one before.

<span class="ok">●</span> <b>Read</b>(crates/oikonomos/src/terminals.rs)
  <span class="dim">└ Read 214 lines</span>

<span class="ok">●</span> <b>Update</b>(crates/oikonomos/src/lib.rs)
  <span class="dim">└ Updated with</span> <span class="add">6 additions</span> <span class="dim">and</span> <span class="del">2 removals</span>

<span class="ok">●</span> <b>Update</b>(web/src/lib/device.ts)
  <span class="dim">└ Updated with</span> <span class="add">14 additions</span> <span class="dim">and</span> <span class="del">3 removals</span>

<span class="run">●</span> <b>Bash</b>(cargo test --workspace)
  <span class="dim">└ Running…
    test terminal_survives_restart ... ok
    test terminal_reconnect ... ok</span>

<span class="accent">* Reattaching…</span> <span class="dim">(34s · esc to interrupt)</span>

<span class="dim">──────────────────────────────────────────</span>
<span class="dim">&gt;</span> <span class="cursor"> </span>
<span class="dim">──────────────────────────────────────────</span>
  <span class="dim">bypass permissions on</span></pre>
{:else}
	<pre><span class="ok">●</span> <b>Read</b>(web/src/lib/device.ts)
  <span class="dim">└ Read 81 lines</span>

<span class="ok">●</span> The retry is a fixed 1 s. I'll add a
  capped exponential backoff.

<span class="warn">●</span> <b>Update</b>(web/src/lib/device.ts)

<span class="dim">────────────────────────────────────────</span>
 <b>Edit file</b> web/src/lib/device.ts

  <span class="del">- if (!closed) setTimeout(open, 1000);</span>
  <span class="add">+ if (!closed)</span>
  <span class="add">+   setTimeout(open, delay());</span>

 Do you want to make this edit?
 <span class="accent">› 1. Yes</span>
   2. Yes, allow all edits
   3. No, and tell Claude what to do
      differently <span class="dim">(esc)</span></pre>
{/if}

<style>
	pre {
		margin: 0;
		padding: var(--space-3) var(--space-4);
		overflow: hidden;
		color: var(--color-fg);
		font-family: var(--font-mono);
		font-size: var(--font-size-md);
		line-height: 1.35;
	}

	.dim {
		color: var(--muted-fg);
	}

	.accent {
		color: var(--ansi-5);
	}

	.ok,
	.add {
		color: var(--ansi-2);
	}

	.del {
		color: var(--ansi-1);
	}

	.warn {
		color: var(--ansi-3);
	}

	.run {
		color: var(--ansi-4);
	}

	.cursor {
		background: var(--color-cursor);
	}
</style>
