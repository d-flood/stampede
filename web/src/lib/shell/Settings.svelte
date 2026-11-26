<script lang="ts" module>
	type Page = 'general' | 'appearance' | 'agents' | 'rigs' | 'clips';

	let open = $state(false);
	let chosen = $state<Page>();

	export function openSettings(page: Page) {
		chosen = page;
		open = true;
	}
</script>

<script lang="ts">
	import { agentNames, autoFetch, clips, commitInstructions, commitRig, fontNames, hostName, modes, rigs, saveAutoFetch, saveCommitInstructions, saveCommitRig, saveHostName, saveModes, themeLabel, type Agent, type FontRole } from '#lib/config.svelte.ts';
	import type { Update } from '#lib/git.ts';
	import { form } from '#lib/form.ts';
	import { colorScheme, selectScheme, type Scheme } from '#lib/scheme.svelte.ts';
	import { selectSidebarSide, sidebarPosition, type SidebarSide } from '#lib/sidebar.svelte.ts';
	import { selectSpinners, spinners, type SpinnerStyle } from '#lib/spinners.svelte.ts';
	import { themeBar, type Slot, type ThemeChoice } from '#lib/themes.svelte.ts';
	import ClipEditor from '#lib/shell/ClipEditor.svelte';
	import RigEditor from '#lib/shell/RigEditor.svelte';
	import FontSwitcher from '#lib/shell/FontSwitcher.svelte';
	import ThemeCards from '#lib/shell/ThemeCards.svelte';
	import ThemeSwitcher from '#lib/shell/ThemeSwitcher.svelte';
	import { defaultSizes, selectTextSize, typography } from '#lib/typography.svelte.ts';
	import Badge from '#lib/ui/Badge.svelte';
	import Button from '#lib/ui/Button.svelte';
	import Dialog from '#lib/ui/Dialog.svelte';
	import IconButton from '#lib/ui/IconButton.svelte';
	import ListItem from '#lib/ui/ListItem.svelte';
	import NavList from '#lib/ui/NavList.svelte';
	import Segmented from '#lib/ui/Segmented.svelte';
	import Select from '#lib/ui/Select.svelte';
	import TextArea from '#lib/ui/TextArea.svelte';
	import TextField from '#lib/ui/TextField.svelte';
	import Toggle from '#lib/ui/Toggle.svelte';
	import type { Component } from 'svelte';
	import BookmarkSimple from '~icons/ph/bookmark-simple';
	import CaretLeft from '~icons/ph/caret-left';
	import CaretRight from '~icons/ph/caret-right';
	import Check from '~icons/ph/check-bold';
	import Gear from '~icons/ph/gear';
	import PaintBrush from '~icons/ph/paint-brush';
	import Robot from '~icons/ph/robot';
	import RocketLaunch from '~icons/ph/rocket-launch';
	import SlidersHorizontal from '~icons/ph/sliders-horizontal';

	let { update }: { update?: Update } = $props();

	const seenKey = 'stampede.update-seen';
	let seen = $state(storedSeen());
	let refusal = $state<string | null>(null);
	let commitRefusal = $state<string | null>(null);
	let modesRefusal = $state<string | null>(null);
	let fetchRefusal = $state<string | null>(null);
	let editingRig = $state(false);
	let docking: ThemeChoice | undefined;
	const agents = Object.keys(agentNames) as Agent[];
	const phone = $derived(form() === 'phone');
	const current = $derived(chosen ?? (phone ? undefined : 'general'));
	const commitRigs = $derived(rigs().filter((rig) => rig.agent !== 'antigravity'));
	const unseen = $derived(update && update.head !== seen);
	const count = (total: number, noun: string) => `${total} ${noun}${total === 1 ? '' : 's'}`;
	const pages: { id: Page; label: string; icon: Component; summary: () => string }[] = [
		{ id: 'general', label: 'General', icon: SlidersHorizontal, summary: () => (update ? 'Update available' : (hostName() ?? 'Stampede')) },
		{ id: 'appearance', label: 'Appearance', icon: PaintBrush, summary: () => themeLabel() },
		{ id: 'agents', label: 'Agents', icon: Robot, summary: () => 'Commit messages and modes' },
		{ id: 'rigs', label: 'Rigs', icon: RocketLaunch, summary: () => count(rigs().length, 'Rig') },
		{ id: 'clips', label: 'Clips', icon: BookmarkSimple, summary: () => count(clips().length, 'Clip') }
	];
	const page = $derived(pages.find(({ id }) => id === current));
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
	const themeSlots: { slot: Slot; label: string }[] = [
		{ slot: 'light', label: 'Light theme' },
		{ slot: 'dark', label: 'Dark theme' }
	];
	const spinnerStyles: { value: SpinnerStyle; label: string }[] = [
		{ value: 'custom', label: 'Custom' },
		{ value: 'standard', label: 'Standard' }
	];

	$effect(() => {
		if (!open || current !== 'general' || !update || update.head === seen) return;
		seen = update.head;
		try {
			localStorage.setItem(seenKey, seen);
		} catch {}
	});

	function storedSeen() {
		try {
			return localStorage.getItem(seenKey);
		} catch {
			return null;
		}
	}

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
	{#if !everywhere}<span class="scope" title="Saved in this browser only; other settings apply on every device"><Badge tone="soft">This browser</Badge></span>{/if}
{/snippet}

<IconButton label={unseen ? 'Settings, Stampede update available' : 'Settings'} title={unseen ? 'Stampede update available' : undefined} aria-haspopup="dialog" aria-expanded={open} onclick={() => ((open = true), phone && (chosen = undefined))}>
	<span class="gear"><Gear />{#if unseen}<span class="dot"></span>{/if}</span>
</IconButton>
{#snippet sections()}
	<NavList aria-label="Settings sections">
		{#each pages as { id, label, icon: Icon, summary } (id)}
			<ListItem element="button" size="md" aria-current={current === id ? 'page' : undefined} onclick={() => (chosen = id)}>
				<span class="icon" class:current={current === id}><Icon /></span>
				<span class="title">{label}{#if phone}<small>{summary()}</small>{/if}</span>
				{#if phone}<span class="chevron"><CaretRight /></span>{/if}
			</ListItem>
		{/each}
	</NavList>
{/snippet}

<Dialog bind:open full={phone} wide fit title="Settings" nav={phone ? undefined : sections} heading={page?.label} oncloseautofocus={dock}>
	<div class="settings" class:phone>
		{#if phone && !current}
			<div class="sections">{@render sections()}</div>
		{:else if current}
			<div class="pane">
				{#if phone && !editingRig}
					<Button variant="link" aria-label="All settings" onclick={() => (chosen = undefined)}><CaretLeft /> Settings</Button>
					<h3>{page?.label}</h3>
				{/if}
				{#if current === 'general'}
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
							<div class="control wide"><TextField id="host-name" placeholder="Stampede" value={hostName() ?? ''} onchange={(event) => rename(event.currentTarget)} aria-describedby="host-name-help" /></div>
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
								<p id="auto-fetch-help">Fetches each Project's origin every half hour and marks default branches a pull would fast-forward. Never pulls.</p>
								{#if fetchRefusal}<p role="alert">{fetchRefusal}</p>{/if}
							</div>
							<div class="control">
								<Toggle aria-labelledby="auto-fetch-label" aria-describedby="auto-fetch-help" bind:checked={autoFetch, chooseAutoFetch} />
							</div>
						</section>
					</div>
				{:else if current === 'appearance'}
					<div class="group">
						<section class="row" aria-labelledby="scheme-label">
							<div class="text">
								<span class="label" id="scheme-label">Color scheme</span>{@render scope(false)}
								<p>System follows this device's light or dark setting.</p>
							</div>
							<div class="control">
								<Segmented aria-labelledby="scheme-label" options={schemes} bind:value={() => colorScheme.choice, selectScheme} />
							</div>
						</section>
						{#each themeSlots as { slot, label } (slot)}
							<section class="row" aria-labelledby="{slot}-theme-label">
								<div class="text">
									<span class="label" id="{slot}-theme-label">{label}</span>{@render scope(true)}
								</div>
								<div class="cards"><ThemeCards {slot} labelledby="{slot}-theme-label" /></div>
							</section>
						{/each}
						<section class="row" aria-labelledby="theme-label">
							<div class="text">
								<span class="label" id="theme-label">Ghostty themes</span>{@render scope(true)}
								<p>Use any Ghostty theme, one for light mode and one for dark. Saved: {themeLabel()}.</p>
							</div>
							<div class="control"><ThemeSwitcher onpreview={(choice) => ((docking = choice), (open = false))} /></div>
						</section>
						<section class="row" aria-labelledby="spinners-label">
							<div class="text">
								<span class="label" id="spinners-label">Spinners</span>{@render scope(false)}
								<p>Custom uses Stampede's dinosaurs. Standard uses the classic spinner, raised hand and dots.</p>
							</div>
							<div class="control">
								<Segmented aria-labelledby="spinners-label" options={spinnerStyles} bind:value={() => spinners.style, selectSpinners} />
							</div>
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
					<div class="group">
						<section class="row" aria-labelledby="commit-rig-label">
							<div class="text">
								<label id="commit-rig-label" for="commit-rig">Commit messages</label>{@render scope(true)}
								<p id="commit-rig-help">The Rig that drafts commit messages from staged changes.</p>
							</div>
							<div class="control">
								<Select id="commit-rig" value={commitRig() ?? ''} disabled={!commitRigs.length} onchange={(event) => chooseCommitRig(event.currentTarget.value)} aria-describedby="commit-rig-help">
									{#each commitRigs as rig (rig.name)}
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
								<p>Offered in each Chat's mode menu and /mode.</p>
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
					<section aria-label="Rigs">
						{#if !editingRig}
							<p class="lede">Starting settings for new chats. Order sets the Ctrl ⇧ shortcuts. {@render scope(true)}</p>
						{/if}
						{#if open}<RigEditor bind:editing={editingRig} />{/if}
					</section>
				{:else}
					<section aria-label="Clips">
						<p class="lede">Text to reuse in any Composer: type ; and a Clip's name to insert it, then edit before sending. {@render scope(true)}</p>
						<div class="group padded">{#if open}<ClipEditor />{/if}</div>
					</section>
				{/if}
			</div>
		{/if}
	</div>
</Dialog>

<style>
	.settings {
		display: flex;
		flex: 1;
		flex-direction: column;
		min-height: 0;
	}

	.sections {
		flex: 1;
		padding: var(--space-2) var(--space-4);
	}

	.icon {
		display: inline-flex;
		flex: none;
		font-size: var(--font-size-lg);
	}

	.icon.current {
		color: var(--accent);
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
		flex: 1;
		min-width: 0;
		min-height: 0;
		padding: 0 var(--space-6) calc(var(--space-6) + var(--space-2));
		overflow: auto;
		mask-image: linear-gradient(#000 calc(100% - var(--space-6)), transparent);
	}

	h3 {
		margin: var(--space-1) 0 var(--space-4);
		font-size: var(--font-size-xl);
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

	.lede:first-child {
		margin-top: 0;
	}

	.group {
		border: var(--border-width) solid var(--border);
		border-radius: var(--radius-lg);
		background: var(--card);
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
		background: var(--accent);
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
		margin-left: var(--space-2);
		color: var(--muted-fg);
		font-weight: 400;
		vertical-align: middle;
	}

	.control {
		box-sizing: border-box;
		flex: none;
		max-width: 100%;
		margin-left: auto;
	}

	.wide {
		flex: 0 1 15rem;
		min-width: 0;
	}

	.cards {
		flex: 1 1 100%;
		max-width: 36rem;
	}

	.font {
		display: flex;
		gap: var(--space-2);
	}

	.control > :global(.select) {
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
		border-radius: var(--radius-pill);
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
		border-color: var(--selected-border);
		background: var(--selected-bg);
		color: var(--color-fg);
	}

	.mode:has(input:checked) > :global(svg) {
		display: inline;
		color: var(--accent);
	}

	.mode:has(input:disabled) {
		cursor: default;
		opacity: 0.7;
	}

	.mode:hover:has(input:enabled) {
		border-color: var(--accent);
	}

	.mode:has(input:focus-visible) {
		outline: var(--focus-ring);
		outline-offset: var(--focus-offset);
	}

	.preview {
		display: grid;
		gap: var(--space-2);
		margin: 0 var(--space-4) var(--space-4);
		padding: var(--space-3) var(--space-4);
		border-radius: var(--radius-md);
		background: var(--plane);
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

	.phone .icon {
		display: grid;
		place-items: center;
		width: 2em;
		height: 2em;
		border-radius: var(--radius-md);
		background: color-mix(in oklab, var(--accent) 16%, var(--surface-2));
		color: var(--accent);
	}

	.chevron {
		display: inline-flex;
		color: var(--muted-fg);
	}

	.phone .pane {
		flex: 1;
		padding: 0 var(--space-4) var(--space-6);
	}

	.phone .wide {
		flex-basis: 100%;
	}
</style>
