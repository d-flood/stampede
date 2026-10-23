<script lang="ts">
	import { agentNames, commitInstructions, commitRig, hostName, modes, rigs, saveCommitInstructions, saveCommitRig, saveHostName, saveModes, themeLabel, uiFont, type Agent } from '#lib/config.svelte.ts';
	import { form } from '#lib/form.ts';
	import { selectSidebarSide, sidebarPosition, type SidebarSide } from '#lib/sidebar.svelte.ts';
	import ClipEditor from '#lib/shell/ClipEditor.svelte';
	import RigEditor from '#lib/shell/RigEditor.svelte';
	import FontSwitcher from '#lib/shell/FontSwitcher.svelte';
	import ThemeSwitcher from '#lib/shell/ThemeSwitcher.svelte';
	import { selectTextSize, textSize, typography } from '#lib/typography.svelte.ts';
	import Button from '#lib/ui/Button.svelte';
	import Dialog from '#lib/ui/Dialog.svelte';
	import IconButton from '#lib/ui/IconButton.svelte';
	import Gear from '~icons/ph/gear';

	let open = $state(false);
	let refusal = $state<string | null>(null);
	let commitRefusal = $state<string | null>(null);
	let modesRefusal = $state<string | null>(null);
	const agents = Object.keys(agentNames) as Agent[];

	async function toggleMode(agent: Agent, input: HTMLInputElement) {
		const chosen = modes(agent).filter((mode) => (mode.value === input.value ? input.checked : mode.available));
		try {
			await saveModes(agent, chosen.map((mode) => mode.value));
			modesRefusal = null;
		} catch (error) {
			input.checked = !input.checked;
			modesRefusal = (error as Error).message;
		}
	}

	async function chooseCommitRig(name: string) {
		try {
			await saveCommitRig(name);
			commitRefusal = null;
		} catch (error) {
			commitRefusal = (error as Error).message;
		}
	}

	async function instruct(textarea: HTMLTextAreaElement) {
		try {
			await saveCommitInstructions(textarea.value);
			commitRefusal = null;
			textarea.value = commitInstructions();
		} catch (error) {
			commitRefusal = (error as Error).message;
		}
	}

	async function rename(input: HTMLInputElement) {
		try {
			await saveHostName(input.value.trim() || null);
			refusal = null;
			input.value = hostName() ?? '';
		} catch (error) {
			refusal = (error as Error).message;
		}
	}
</script>

<IconButton label="Settings" aria-haspopup="dialog" aria-expanded={open} onclick={() => (open = true)}><Gear /></IconButton>
<Dialog bind:open full={form() === 'phone'} title="Settings">
	<div class="settings">
		<section aria-labelledby="host-name-label">
			<div class="row">
				<label id="host-name-label" for="host-name">Host name</label>
				<input id="host-name" type="text" placeholder="Stampede" value={hostName() ?? ''} onchange={(event) => rename(event.currentTarget)} aria-describedby="host-name-help" />
			</div>
			<p id="host-name-help">Names this Host's installed app and windows on every device. Installed apps pick up a new name within about a day.</p>
			{#if refusal}<p role="alert">{refusal}</p>{/if}
		</section>
		<section aria-labelledby="text-size-label">
			<div class="row">
				<label id="text-size-label" for="text-size">Text size</label>
				<select id="text-size" value={textSize()} onchange={(event) => selectTextSize(Number(event.currentTarget.value))} aria-describedby="text-size-help">
					{#each Array.from({ length: 11 }, (_, index) => index + 12) as size}
						<option value={size}>{size} px</option>
					{/each}
				</select>
			</div>
			<p id="text-size-help">Applies to the UI, chats, editor, and terminals. Saved in this browser.</p>
			<div class="preview" aria-label="Text preview">
				<span>The quick brown fox jumps over the lazy dog.</span>
				<code>const greeting = 'Hello, world!';</code>
				<small>Labels and supporting text</small>
			</div>
			<div class="row">
				<p>{typography.size === null ? 'Using default: 16 px on mobile, 13 px on desktop.' : 'Custom text size'}</p>
				<Button disabled={typography.size === null} onclick={() => selectTextSize(null)}>Reset</Button>
			</div>
		</section>
		<section aria-labelledby="sidebar-side-label">
			<div class="row">
				<label id="sidebar-side-label" for="sidebar-side">Sidebar position</label>
				<select id="sidebar-side" value={sidebarPosition.side} onchange={(event) => selectSidebarSide(event.currentTarget.value as SidebarSide)} aria-describedby="sidebar-side-help">
					<option value="left">Left</option>
					<option value="right">Right</option>
				</select>
			</div>
			<p id="sidebar-side-help">Places the desktop sidebar on the left or right. Saved in this browser.</p>
		</section>
		<section aria-labelledby="theme-label">
			<div class="row">
				<div><h3 id="theme-label">Theme</h3><p>{themeLabel()}</p></div>
				<ThemeSwitcher />
			</div>
			<p>Light or dark follows your system unless the color scheme button beside Settings picks one. Applies on every device.</p>
		</section>
		<section aria-labelledby="font-label">
			<div class="row">
				<div><h3 id="font-label">Font</h3><p>{uiFont.family ?? 'Default'}</p></div>
				<FontSwitcher />
			</div>
			<p>For the UI and chat text on every device. Code, paths, and terminals keep the monospace font.</p>
		</section>
		<section aria-labelledby="commit-rig-label">
			<div class="row wrap">
				<label id="commit-rig-label" for="commit-rig">Commit messages</label>
				<select id="commit-rig" value={commitRig() ?? ''} disabled={!rigs().length} onchange={(event) => chooseCommitRig(event.currentTarget.value)} aria-describedby="commit-rig-help">
					{#each rigs() as rig (rig.name)}
						<option value={rig.name}>{rig.name}</option>
					{:else}
						<option value="">No Rigs</option>
					{/each}
				</select>
			</div>
			<p id="commit-rig-help">The Rig that drafts commit messages from staged changes.</p>
			<label class="field" for="commit-instructions">Instructions</label>
			<textarea id="commit-instructions" rows="4" value={commitInstructions()} onchange={(event) => instruct(event.currentTarget)} aria-describedby="commit-instructions-help"></textarea>
			<p id="commit-instructions-help">The Rig reads only these, recent commit subjects and the staged diff: no tools, settings or AGENTS.md files (Codex still reads its AGENTS.md). Clear to restore the default.</p>
			{#if commitRefusal}<p role="alert">{commitRefusal}</p>{/if}
		</section>
		<section aria-labelledby="modes-label">
			<h3 id="modes-label">Modes</h3>
			<p>The modes each Agent's Chats offer in the mode menu, /mode and Shift+Tab. Saved on every device.</p>
			{#each agents as agent (agent)}
				{@const offered = modes(agent)}
				<fieldset>
					<legend>{agentNames[agent]}</legend>
					{#each offered as mode (mode.value)}
						<label class="mode">
							<input type="checkbox" value={mode.value} checked={mode.available} disabled={mode.available && offered.filter((other) => other.available).length === 1} onchange={(event) => toggleMode(agent, event.currentTarget)} />
							{mode.label}
						</label>
					{/each}
				</fieldset>
			{/each}
			{#if modesRefusal}<p role="alert">{modesRefusal}</p>{/if}
		</section>
		<section aria-labelledby="rigs-label">
			<h3 id="rigs-label">Rigs</h3>
			<p>Starting settings for new chats. Saved on every device.</p>
			{#if open}<RigEditor />{/if}
		</section>
		<section aria-labelledby="clips-label">
			<h3 id="clips-label">Clips</h3>
			<p>Text to reuse in any Composer: type ; and a Clip's name to insert it, then edit before sending. Saved on every device.</p>
			{#if open}<ClipEditor />{/if}
		</section>
	</div>
	{#snippet actions()}
		<Button onclick={() => (open = false)}>Done</Button>
	{/snippet}
</Dialog>

<style>
	.settings {
		display: grid;
		gap: var(--space-6);
	}

	section + section {
		padding-top: var(--space-4);
		border-top: var(--border-width) solid var(--border);
	}

	.row {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: var(--space-3);
	}

	.wrap {
		flex-wrap: wrap;
	}

	label,
	h3 {
		margin: 0;
		font-size: var(--font-size-md);
		font-weight: 600;
	}

	p {
		margin: var(--space-2) 0;
		color: var(--muted-fg);
		font-size: var(--font-size-sm);
		line-height: 1.5;
	}

	.field {
		display: block;
		margin-top: var(--space-3);
		font-size: var(--font-size-sm);
	}

	textarea {
		display: block;
		box-sizing: border-box;
		width: 100%;
		margin-top: var(--space-2);
		resize: vertical;
		line-height: 1.5;
	}

	select,
	input,
	textarea {
		min-height: var(--target-min);
		padding: var(--space-1) var(--space-2);
		border: var(--border-width) solid var(--border);
		border-radius: var(--radius-md);
		background: var(--color-bg);
		color: var(--color-fg);
		font: inherit;
	}

	input {
		min-width: 0;
	}

	select:focus-visible,
	input:focus-visible,
	textarea:focus-visible {
		outline: var(--border-width) solid var(--status-working);
		outline-offset: var(--border-width);
	}

	fieldset {
		display: flex;
		flex-wrap: wrap;
		gap: 0 var(--space-4);
		margin: var(--space-3) 0 0;
		padding: 0;
		border: none;
	}

	legend {
		padding: 0;
		font-size: var(--font-size-sm);
		font-weight: 600;
	}

	.mode {
		display: inline-flex;
		align-items: center;
		gap: var(--space-2);
		min-height: var(--target-min);
		font-weight: normal;
	}

	.mode input {
		min-height: 0;
		margin: 0;
		padding: 0;
		accent-color: var(--status-working);
	}

	.preview {
		display: grid;
		gap: var(--space-3);
		margin: var(--space-4) 0;
		padding: var(--space-4);
		border: var(--border-width) solid var(--border);
		border-radius: var(--radius-md);
		background: var(--color-bg);
		line-height: 1.5;
		overflow-wrap: anywhere;
	}

	code {
		font-family: var(--font-mono);
	}

	small {
		color: var(--muted-fg);
		font-size: var(--font-size-sm);
	}
</style>
