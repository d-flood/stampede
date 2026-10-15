<script lang="ts">
	type Row = { old?: [number, string]; new?: [number, string]; kind?: 'change' | 'add' };

	const rows: Row[] = [
		{ old: [38, 'export function connect('], new: [38, 'export function connect('] },
		{ old: [39, '\thandlers: Handlers,'], new: [39, '\thandlers: Handlers,'] },
		{ old: [40, '): Device {'], new: [40, '): Device {'] },
		{ old: [41, '\tconst id = deviceId();'], new: [41, '\tconst id = deviceId();'] },
		{ old: [42, '\tlet socket: WebSocket;'], new: [42, '\tlet socket: WebSocket;'] },
		{ old: [43, '\tlet closed = false;'], new: [43, '\tlet closed = false;'] },
		{ new: [44, '\tlet attempt = 0;'], kind: 'add' },
		{ old: [44, ''], new: [45, ''] },
		{ new: [46, '\tfunction delay() {'], kind: 'add' },
		{ new: [47, '\t\tattempt += 1;'], kind: 'add' },
		{ new: [48, '\t\treturn Math.min('], kind: 'add' },
		{ new: [49, '\t\t\t1000 * 2 ** attempt,'], kind: 'add' },
		{ new: [50, '\t\t\t30_000,'], kind: 'add' },
		{ new: [51, '\t\t);'], kind: 'add' },
		{ new: [52, '\t}'], kind: 'add' },
		{ new: [53, ''], kind: 'add' },
		{ old: [45, '\tfunction open() {'], new: [54, '\tfunction open() {'] },
		{ old: [46, '\t\tsocket = new WebSocket(url);'], new: [55, '\t\tsocket = new WebSocket(url);'] },
		{ old: [47, '\t\tsocket.onopen = hello;'], new: [56, '\t\tsocket.onopen = () => {'], kind: 'change' },
		{ new: [57, '\t\t\tattempt = 0;'], kind: 'add' },
		{ new: [58, '\t\t\thello();'], kind: 'add' },
		{ new: [59, '\t\t};'], kind: 'add' },
		{ old: [48, '\t\tsocket.onclose = () => {'], new: [60, '\t\tsocket.onclose = () => {'] },
		{ new: [61, "\t\t\tstatus('reconnecting');"], kind: 'add' },
		{ old: [49, '\t\t\tif (closed) return;'], new: [62, '\t\t\tif (closed) return;'] },
		{ old: [50, '\t\t\tsetTimeout(open, 1000);'], new: [63, '\t\t\tsetTimeout(open, delay());'], kind: 'change' },
		{ old: [51, '\t\t};'], new: [64, '\t\t};'] },
		{ old: [52, '\t}'], new: [65, '\t}'] }
	];

	const start = (n: number) => !!rows[n].kind && !rows[n - 1]?.kind;
</script>

<div class="diff">
	{#each rows as row, n (n)}
		<div class="row">
			<span class="no">{row.old?.[0] ?? ''}</span>
			<code class="old" class:del={row.kind === 'change'} class:empty={!row.old}>{row.old?.[1] ?? ''}</code>
			<span class="no">
				{#if start(n)}<button class="revert" aria-label="Revert change">↶</button>{/if}
				{row.new?.[0] ?? ''}
			</span>
			<code class="new" class:add={row.kind} class:empty={!row.new}>{row.new?.[1] ?? ''}</code>
		</div>
	{/each}
</div>

<style>
	.diff {
		padding: var(--space-2) 0;
		overflow: hidden;
		font-family: var(--font-mono);
		font-size: var(--font-size-md);
		line-height: 1.5;
		tab-size: 2;
	}

	.row {
		display: grid;
		grid-template-columns: 4.5ch minmax(0, 1fr) 4.5ch minmax(0, 1fr);
	}

	.no {
		position: relative;
		padding-right: var(--space-2);
		color: var(--muted-fg);
		text-align: right;
		user-select: none;
	}

	code {
		padding: 0 var(--space-2);
		overflow: hidden;
		font: inherit;
		white-space: pre;
	}

	.old {
		border-right: var(--border-width) solid var(--border);
	}

	.del {
		background: color-mix(in oklab, var(--ansi-1) 22%, var(--color-bg));
	}

	.add {
		background: color-mix(in oklab, var(--ansi-2) 22%, var(--color-bg));
	}

	.empty {
		background: repeating-linear-gradient(-45deg, transparent 0 4px, var(--surface-2) 4px 5px);
	}

	.revert {
		position: absolute;
		left: 0;
		padding: 0;
		border: none;
		background: none;
		color: var(--muted-fg);
		font: inherit;
		cursor: pointer;
	}
</style>
