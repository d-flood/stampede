<script lang="ts">
	import { agentNames, autoFetch, clips, commitInstructions, commitRig, fontNames, hostName, modes, rigs, saveAutoFetch, saveCommitInstructions, saveCommitRig, saveHostName, saveModes, themeLabel, type Agent, type FontRole } from '#lib/config.svelte.ts';
	import type { Update } from '#lib/git.ts';
	import { form } from '#lib/form.ts';
	import { colorScheme, selectScheme, type Scheme } from '#lib/scheme.svelte.ts';
	import { selectSidebarSide, sidebarPosition, type SidebarSide } from '#lib/sidebar.svelte.ts';
	import { themeBar, type ThemeChoice } from '#lib/themes.svelte.ts';
	import ClipEditor from '#lib/shell/ClipEditor.svelte';
	import RigEditor from '#lib/shell/RigEditor.svelte';
	import FontSwitcher from '#lib/shell/FontSwitcher.svelte';
	import ThemeSwitcher from '#lib/shell/ThemeSwitcher.svelte';
	import { defaultSizes, selectTextSize, typography } from '#lib/typography.svelte.ts';
	import Button from '#lib/ui/Button.svelte';
	import Dialog from '#lib/ui/Dialog.svelte';
	import IconButton from '#lib/ui/IconButton.svelte';
	import Select from '#lib/ui/Select.svelte';
	import TextArea from '#lib/ui/TextArea.svelte';
	import TextField from '#lib/ui/TextField.svelte';
	import type { Component } from 'svelte';
	import BookmarkSimple from '~icons/ph/bookmark-simple';
	import Browser from '~icons/ph/browser';
	import CaretLeft from '~icons/ph/caret-left';
	import CaretRight from '~icons/ph/caret-right';
	import Check from '~icons/ph/check-bold';
	import Devices from '~icons/ph/devices';
	import Gear from '~icons/ph/gear';
	import PaintBrush from '~icons/ph/paint-brush';
	import Robot from '~icons/ph/robot';
	import RocketLaunch from '~icons/ph/rocket-launch';
	import SlidersHorizontal from '~icons/ph/sliders-horizontal';

	type Page = 'general' | 'appearance' | 'agents' | 'rigs' | 'clips';

	let { update }: { update?: Update } = $props();

	let open = $state(false);
	let chosen = $state<Page>();
	let refusal = $state<string | null>(null);
	let commitRefusal = $state<string | null>(null);
	let modesRefusal = $state<string | null>(null);
	let fetchRefusal = $state<string | null>(null);
	let docking: ThemeChoice | undefined;
	const agents = Object.keys(agentNames) as Agent[];
	const phone = $derived(form() === 'phone');
	const current = $derived(chosen ?? (phone ? undefined : 'general'));
	const count = (total: number, noun: string) => `${total} ${noun}${total === 1 ? '' : 's'}`;
	const pages: { id: Page; label: string; icon: Component; summary: () => string }[] = [
		{ id: 'general', label: 'General', icon: SlidersHorizontal, summary: () => (update ? 'Update available' : (hostName() ?? 'Stampede')) },
		{ id: 'appearance', label: 'Appearance', icon: PaintBrush, summary: () => themeLabel() },
		{ id: 'agents', label: 'Agents', icon: Robot, summary: () => 'Commit messages and modes' },
		{ id: 'rigs', label: 'Rigs', icon: RocketLaunch, summary: () => count(rigs().length, 'Rig') },
		{ id: 'clips', label: 'Clips', icon: BookmarkSimple, summary: () => count(clips().length, 'Clip') }
	];
	const roles: { role: FontRole; description: string }[] = [
		{ role: 'ui', description: 'Menus, buttons, tabs and labels.' },
		{ role: 'chat', description: 'Rendered markdown: chat messages, Epic briefs and markdown previews.' },
		{ role: 'code', description: 'The editor, terminals, file names, code blocks and inline code.' }
	];
	const sizes = Array.from({ length: 19 }, (_, index) => index + 10);
	const schemes: { value: Scheme; label: string }[] = [
		{ value: 'system', label: 'System' },
		{ value: 'light', label: 'Light' },
		{ value: 'dark', label: 'Dark' }
	];

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

	async function chooseAutoFetch(on: boolean) {
		try {
			await saveAutoFetch(on);
			fetchRefusal = null;
		} catch (error) {
			fetchRefusal = (error as Error).message;
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

	function dock(event: Event) {
		if (!docking) return;
		event.preventDefault();
		themeBar.choice = docking;
		docking = undefined;
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

{#snippet scope(everywhere: boolean)}
	<span class="scope" title={everywhere ? 'Saved on every device' : 'Saved in this browser'}>
		{#if everywhere}<Devices /> All devices{:else}<Browser /> This browser{/if}
	</span>
{/snippet}

<IconButton label={update ? 'Settings, Stampede update available' : 'Settings'} title={update ? 'Stampede update available' : undefined} aria-haspopup="dialog" aria-expanded={open} onclick={() => ((open = true), phone && (chosen = undefined))}>
	<span class="gear"><Gear />{#if update}<span class="dot"></span>{/if}</span>
</IconButton>
<Dialog bind:open full={phone} wide title="Settings" oncloseautofocus={dock}>
	<div class="settings" class:phone>
		{#if !phone || !current}
			<nav aria-label="Settings sections">
				{#each pages as { id, label, icon: Icon, summary } (id)}
					<button aria-current={current === id ? 'page' : undefined} onclick={() => (chosen = id)}>
						<span class="icon"><Icon /></span>
						<span class="title">{label}{#if phone}<small>{summary()}</small>{/if}</span>
						{#if phone}<CaretRight />{/if}
					</button>
				{/each}
			</nav>
		{/if}
		{#if current}
			<div class="pane">
				{#if phone}<button class="back" aria-label="All settings" onclick={() => (chosen = undefined)}><CaretLeft /> Settings</button>{/if}
				{#if current === 'general'}
					<h3>General</h3>
					{#if update}
						<section class="group row update" aria-labelledby="update-label">
							<div class="text">
								<span class="label" id="update-label">Stampede update</span>
								<p>{update.pulled ? `Pulled ${count(update.behind, 'new commit')}. Run ./rebuild to apply them.` : `${count(update.behind, 'new commit')} on origin. Pull, then run ./rebuild.`}</p>
								<ul>
									{#each update.subjects as subject, index (index)}<li>{subject}</li>{/each}
									{#if update.behind > update.subjects.length}<li>and {update.behind - update.subjects.length} more</li>{/if}
								</ul>
							</div>
						</section>
					{/if}
					<div class="group">
						<section class="row" aria-labelledby="host-name-label">
							<div class="text">
								<label id="host-name-label" for="host-name">Host name</label>{@render scope(true)}
								<p id="host-name-help">Names the installed app and windows. Installed apps pick up a new name within about a day.</p>
								{#if refusal}<p role="alert">{refusal}</p>{/if}
							</div>
							<div class="control"><TextField id="host-name" placeholder="Stampede" value={hostName() ?? ''} onchange={(event) => rename(event.currentTarget)} aria-describedby="host-name-help" /></div>
						</section>
						<section class="row" aria-labelledby="sidebar-side-label">
							<div class="text">
								<label id="sidebar-side-label" for="sidebar-side">Sidebar position</label>{@render scope(false)}
								<p id="sidebar-side-help">Left or right of the tabs, including on phones.</p>
							</div>
							<div class="control">
								<Select id="sidebar-side" value={sidebarPosition.side} onchange={(event) => selectSidebarSide(event.currentTarget.value as SidebarSide)} aria-describedby="sidebar-side-help">
									<option value="left">Left</option>
									<option value="right">Right</option>
								</Select>
							</div>
						</section>
						<section class="row" aria-labelledby="auto-fetch-label">
							<div class="text">
								<span class="label" id="auto-fetch-label">Check origin</span>{@render scope(true)}
								<p>Fetches each Project's origin every half hour and marks default branches a pull would fast-forward. Never pulls.</p>
								{#if fetchRefusal}<p role="alert">{fetchRefusal}</p>{/if}
							</div>
							<div class="segments control" role="radiogroup" aria-labelledby="auto-fetch-label">
								{#each [true, false] as on (on)}
									<button role="radio" aria-checked={autoFetch() === on} onclick={() => chooseAutoFetch(on)}>{on ? 'On' : 'Off'}</button>
								{/each}
							</div>
						</section>
					</div>
				{:else if current === 'appearance'}
					<h3>Appearance</h3>
					<div class="group">
						<section class="row" aria-labelledby="scheme-label">
							<div class="text">
								<span class="label" id="scheme-label">Color scheme</span>{@render scope(false)}
								<p>System follows this device's light or dark setting.</p>
							</div>
							<div class="segments control" role="radiogroup" aria-labelledby="scheme-label">
								{#each schemes as { value, label } (value)}
									<button role="radio" aria-checked={colorScheme.choice === value} onclick={() => selectScheme(value)}>{label}</button>
								{/each}
							</div>
						</section>
						<section class="row" aria-labelledby="theme-label">
							<div class="text">
								<span class="label" id="theme-label">Theme</span>{@render scope(true)}
								<p>One theme for light mode and one for dark.</p>
							</div>
							<div class="control"><ThemeSwitcher onpreview={(choice) => ((docking = choice), (open = false))} /></div>
						</section>
					</div>
					<h4 id="fonts-label">Fonts</h4>
					<p class="lede">Fonts are saved on every device. Sizes are saved in this browser.</p>
					<section class="group" aria-labelledby="fonts-label">
						{#each roles as { role, description } (role)}
							<section class="row" aria-labelledby="{role}-font-label">
								<div class="text">
									<span class="label" id="{role}-font-label">{fontNames[role]}</span>
									<p>{description}</p>
								</div>
								<div class="control font">
									<FontSwitcher {role} />
									<Select aria-label="{fontNames[role]} size" value={typography[role] ?? ''} onchange={(event) => selectTextSize(role, event.currentTarget.value ? Number(event.currentTarget.value) : null)}>
										<option value="">Default · {defaultSizes[role][phone ? 'phone' : 'desktop']} px</option>
										{#each sizes as size (size)}<option value={size}>{size} px</option>{/each}
									</Select>
								</div>
							</section>
						{/each}
						<div class="preview" aria-label="Font preview">
							<span>Open Project · Commit · Settings</span>
							<span class="chat">The quick brown fox jumps over the lazy dog.</span>
							<code>const greeting = 'Hello, world!';</code>
						</div>
					</section>
				{:else if current === 'agents'}
					<h3>Agents</h3>
					<div class="group">
						<section class="row" aria-labelledby="commit-rig-label">
							<div class="text">
								<label id="commit-rig-label" for="commit-rig">Commit messages</label>{@render scope(true)}
								<p id="commit-rig-help">The Rig that drafts commit messages from staged changes.</p>
							</div>
							<div class="control">
								<Select id="commit-rig" value={commitRig() ?? ''} disabled={!rigs().length} onchange={(event) => chooseCommitRig(event.currentTarget.value)} aria-describedby="commit-rig-help">
									{#each rigs() as rig (rig.name)}
										<option value={rig.name}>{rig.name}</option>
									{:else}
										<option value="">No Rigs</option>
									{/each}
								</Select>
							</div>
							<div class="field">
								<label for="commit-instructions">Instructions</label>
								<TextArea id="commit-instructions" rows={4} value={commitInstructions()} onchange={(event) => instruct(event.currentTarget)} aria-describedby="commit-instructions-help" />
								<p id="commit-instructions-help">The Rig reads only these, recent commit subjects and the staged diff: no tools, settings or AGENTS.md files (Codex still reads its AGENTS.md). Clear to restore the default.</p>
								{#if commitRefusal}<p role="alert">{commitRefusal}</p>{/if}
							</div>
						</section>
						<section class="row" aria-labelledby="modes-label">
							<div class="text">
								<span class="label" id="modes-label">Modes</span>{@render scope(true)}
								<p>Offered in each Chat's mode menu, /mode and Shift+Tab.</p>
							</div>
							<div class="field">
								{#each agents as agent (agent)}
									{@const offered = modes(agent)}
									<fieldset>
										<legend>{agentNames[agent]}</legend>
										{#each offered as mode (mode.value)}
											<label class="mode">
												<input type="checkbox" value={mode.value} checked={mode.available} disabled={mode.available && offered.filter((other) => other.available).length === 1} onchange={(event) => toggleMode(agent, event.currentTarget)} />
												<Check />
												<span>{mode.label}</span>
											</label>
										{/each}
									</fieldset>
								{/each}
								{#if modesRefusal}<p role="alert">{modesRefusal}</p>{/if}
							</div>
						</section>
					</div>
				{:else if current === 'rigs'}
					<section aria-labelledby="rigs-label">
						<h3 id="rigs-label">Rigs</h3>
						<p class="lede">Starting settings for new chats. {@render scope(true)}</p>
						<div class="group padded">{#if open}<RigEditor />{/if}</div>
					</section>
				{:else}
					<section aria-labelledby="clips-label">
						<h3 id="clips-label">Clips</h3>
						<p class="lede">Text to reuse in any Composer: type ; and a Clip's name to insert it, then edit before sending. {@render scope(true)}</p>
						<div class="group padded">{#if open}<ClipEditor />{/if}</div>
					</section>
				{/if}
			</div>
		{/if}
	</div>
	{#snippet actions()}
		<Button onclick={() => (open = false)}>Done</Button>
	{/snippet}
</Dialog>

<style>
	.settings {
		display: grid;
		flex: 1;
		grid-template-columns: 12rem minmax(0, 1fr);
		min-height: 0;
	}

	nav {
		display: flex;
		flex-direction: column;
		gap: 2px;
		padding: var(--space-3) var(--space-2);
		overflow: auto;
		border-right: var(--border-width) solid var(--border);
	}

	nav button {
		display: flex;
		align-items: center;
		gap: var(--space-3);
		min-height: calc(var(--target-min) + 6px);
		padding: 0 var(--space-3);
		border: none;
		border-radius: var(--radius-md);
		background: none;
		color: var(--muted-fg);
		font: inherit;
		text-align: start;
		cursor: pointer;
		transition:
			background var(--dur-fast),
			color var(--dur-fast);
	}

	nav button:hover {
		background: var(--hover);
		color: var(--color-fg);
	}

	nav button:focus-visible,
	.back:focus-visible,
	.segments button:focus-visible {
		outline: 2px solid color-mix(in oklab, var(--status-working) 70%, transparent);
		outline-offset: -2px;
	}

	nav [aria-current='page'] {
		background: var(--color-bg);
		box-shadow:
			var(--shadow-lift),
			0 0 0 var(--border-width) var(--border);
		color: var(--color-fg);
		font-weight: 600;
	}

	.icon {
		display: inline-flex;
		flex: none;
		font-size: var(--font-size-lg);
	}

	[aria-current='page'] .icon {
		color: var(--status-working);
	}

	.title {
		display: grid;
		flex: 1;
		min-width: 0;
	}

	.title small {
		overflow: hidden;
		color: var(--muted-fg);
		font-size: var(--font-size-sm);
		font-weight: normal;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.pane {
		min-width: 0;
		padding: var(--space-4) var(--space-6) var(--space-6);
		overflow: auto;
	}

	h3 {
		margin: var(--space-1) 0 var(--space-4);
		font-size: var(--font-size-lg);
	}

	h4 {
		margin: var(--space-6) 0 var(--space-3);
		font-size: var(--font-size-md);
	}

	.lede {
		margin: calc(-1 * var(--space-2)) 0 var(--space-4);
		color: var(--muted-fg);
		font-size: var(--font-size-sm);
		line-height: 1.5;
	}

	.group {
		border: var(--border-width) solid var(--border);
		border-radius: var(--radius-lg);
		background: var(--color-bg);
	}

	.update {
		margin-bottom: var(--space-4);
	}

	ul {
		margin: var(--space-2) 0 0;
		padding-left: 1.2em;
		color: var(--muted-fg);
		font-size: var(--font-size-sm);
		line-height: 1.5;
	}

	.gear {
		position: relative;
		display: inline-grid;
	}

	.dot {
		position: absolute;
		top: -1px;
		right: -2px;
		width: 6px;
		height: 6px;
		border-radius: 50%;
		background: var(--status-working);
	}

	.padded {
		padding: var(--space-4);
	}

	.row {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		justify-content: space-between;
		gap: var(--space-3) var(--space-6);
		padding: var(--space-4);
	}

	.row + .row {
		border-top: var(--border-width) solid var(--border);
	}

	.text {
		flex: 1 1 14rem;
		min-width: 0;
	}

	.text label,
	.label {
		font-weight: 600;
	}

	p {
		margin: var(--space-1) 0 0;
		color: var(--muted-fg);
		font-size: var(--font-size-sm);
		line-height: 1.5;
	}

	[role='alert'] {
		color: var(--ansi-1);
	}

	.scope {
		display: inline-flex;
		align-items: center;
		gap: 3px;
		margin-left: var(--space-2);
		padding: 0 var(--space-2);
		border: var(--border-width) solid var(--border);
		border-radius: 999px;
		color: var(--muted-fg);
		font-size: calc(var(--font-size-sm) - 1px);
		font-weight: normal;
		line-height: 1.6;
		vertical-align: 1px;
		white-space: nowrap;
	}

	.control {
		box-sizing: border-box;
		flex: 0 1 15rem;
		min-width: 0;
	}

	.font {
		display: flex;
		flex-basis: 21rem;
		gap: var(--space-2);
	}

	.font > :global(select) {
		flex: none;
		width: auto;
	}

	.field {
		display: grid;
		flex-basis: 100%;
		gap: var(--space-2);
		min-width: 0;
	}

	.field > label {
		font-size: var(--font-size-sm);
		font-weight: 600;
	}

	.segments {
		display: flex;
		padding: 2px;
		border-radius: calc(var(--radius-md) + 2px);
		background: var(--surface-2);
		box-shadow: inset 0 0 0 var(--border-width) var(--border);
	}

	.segments button {
		flex: 1;
		min-height: var(--target-min);
		border: none;
		border-radius: var(--radius-md);
		background: none;
		color: var(--muted-fg);
		font: inherit;
		cursor: pointer;
	}

	.segments button:hover {
		color: var(--color-fg);
	}

	.segments [aria-checked='true'] {
		background: var(--color-bg);
		box-shadow:
			var(--shadow-lift),
			0 0 0 var(--border-width) var(--border);
		color: var(--color-fg);
		font-weight: 600;
	}

	fieldset {
		display: flex;
		flex-wrap: wrap;
		gap: var(--space-2);
		margin: var(--space-2) 0 0;
		padding: 0;
		border: none;
	}

	legend {
		margin-bottom: var(--space-2);
		padding: 0;
		font-size: var(--font-size-sm);
		font-weight: 600;
	}

	.mode {
		position: relative;
		display: inline-flex;
		align-items: center;
		gap: var(--space-1);
		min-height: var(--target-min);
		padding: 0 var(--space-3);
		border: var(--border-width) solid var(--border-strong);
		border-radius: 999px;
		color: var(--muted-fg);
		cursor: pointer;
		transition:
			background var(--dur-fast),
			border-color var(--dur-fast),
			color var(--dur-fast);
	}

	.mode span::first-letter {
		text-transform: uppercase;
	}

	.mode input {
		position: absolute;
		inset: 0;
		margin: 0;
		opacity: 0;
		cursor: inherit;
	}

	.mode > :global(svg) {
		display: none;
		font-size: 0.85em;
	}

	.mode:has(input:checked) {
		border-color: color-mix(in oklab, var(--status-working) 55%, var(--border));
		background: color-mix(in oklab, var(--status-working) 14%, transparent);
		color: var(--color-fg);
	}

	.mode:has(input:checked) > :global(svg) {
		display: inline;
		color: var(--status-working);
	}

	.mode:has(input:disabled) {
		cursor: default;
		opacity: 0.7;
	}

	.mode:hover:has(input:enabled) {
		border-color: var(--status-working);
	}

	.mode:has(input:focus-visible) {
		outline: 2px solid color-mix(in oklab, var(--status-working) 70%, transparent);
		outline-offset: 1px;
	}

	.preview {
		display: grid;
		gap: var(--space-2);
		margin: 0 var(--space-4) var(--space-4);
		padding: var(--space-3) var(--space-4);
		border-radius: var(--radius-md);
		background: var(--surface-1);
		line-height: 1.5;
		overflow-wrap: anywhere;
	}

	.chat {
		font-family: var(--font-chat);
		font-size: var(--font-size-chat);
	}

	code {
		font-family: var(--font-mono);
		font-size: var(--font-size-code);
	}

	.phone {
		display: flex;
		flex-direction: column;
	}

	.phone nav {
		flex: 1;
		gap: 0;
		padding: var(--space-2) var(--space-4);
		border: none;
	}

	.phone nav button {
		gap: var(--space-4);
		min-height: calc(var(--target-min) + 16px);
		padding: var(--space-2) 0;
		border-radius: 0;
		color: var(--color-fg);
	}

	.phone nav button + button {
		border-top: var(--border-width) solid var(--border);
	}

	.phone nav button:hover {
		background: none;
	}

	.phone .icon {
		display: grid;
		place-items: center;
		width: 2em;
		height: 2em;
		border-radius: var(--radius-md);
		background: color-mix(in oklab, var(--status-working) 16%, var(--surface-2));
		color: var(--status-working);
	}

	.phone nav > button > :global(svg) {
		flex: none;
		color: var(--muted-fg);
	}

	.phone .pane {
		flex: 1;
		padding: 0 var(--space-4) var(--space-6);
	}

	.back {
		display: inline-flex;
		align-items: center;
		gap: var(--space-1);
		min-height: var(--target-min);
		margin-left: calc(-1 * var(--space-2));
		padding: 0 var(--space-2);
		border: none;
		border-radius: var(--radius-md);
		background: none;
		color: var(--status-working);
		font: inherit;
		cursor: pointer;
	}

	.phone .control {
		flex-basis: 100%;
	}
</style>
