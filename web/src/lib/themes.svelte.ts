import { beginThemePreview, selectedTheme, selectTheme } from '#lib/config.svelte.ts';
import { isDark } from '#lib/scheme.svelte.ts';

export type Slot = 'light' | 'dark';

export function other(slot: Slot): Slot {
	return slot === 'light' ? 'dark' : 'light';
}

export class ThemeChoice {
	names = $state<string[]>([]);
	query = $state('');
	loading = $state(true);
	saving = $state(false);
	pending = $state(false);
	error = $state<string>();
	scheme = $state<Slot>(isDark() ? 'dark' : 'light');
	chosen = $state({ light: selectedTheme.light, dark: selectedTheme.dark });
	previewed = $state({ light: selectedTheme.light, dark: selectedTheme.dark });
	#preview = beginThemePreview();
	#request = 0;

	constructor() {
		this.load();
	}

	// Getters rather than $derived: a choice outlives the picker that creates it, and Svelte freezes deriveds whose owning effect is destroyed.
	get matches() {
		return this.names.filter((name) => name.toLowerCase().includes(this.query.toLowerCase().trim()));
	}

	get candidate() {
		return this.chosen[this.scheme] ?? this.chosen[other(this.scheme)];
	}

	get spec() {
		const light = this.chosen.light ?? this.chosen.dark;
		const dark = this.chosen.dark ?? this.chosen.light;
		return light === dark ? light : `light:${light},dark:${dark}`;
	}

	get ready() {
		return !this.saving && !this.pending && this.spec !== null && this.chosen.light === this.previewed.light && this.chosen.dark === this.previewed.dark;
	}

	async load() {
		this.loading = true;
		this.error = undefined;
		try {
			const response = await fetch('/api/themes');
			if (!response.ok) throw new Error('Could not load themes.');
			this.names = await response.json();
		} catch (failure) {
			this.error = failure instanceof Error ? failure.message : 'Could not load themes.';
		} finally {
			this.loading = false;
		}
	}

	async #show(name: string) {
		const current = ++this.#request;
		this.pending = true;
		this.error = undefined;
		try {
			return await this.#preview.preview(name);
		} catch (failure) {
			this.error = failure instanceof Error ? failure.message : 'Could not preview theme.';
			return false;
		} finally {
			if (current === this.#request) this.pending = false;
		}
	}

	async choose(name: string) {
		if (this.saving) return;
		const slot = this.scheme;
		this.chosen[slot] = name;
		if (await this.#show(name)) this.previewed[slot] = name;
	}

	pick(slot: Slot) {
		this.scheme = slot;
		if (this.candidate) this.#show(this.candidate);
	}

	cycle(direction: number) {
		const { matches } = this;
		const index = matches.indexOf(this.chosen[this.scheme] ?? '');
		const next = index < 0 ? (direction > 0 ? 0 : matches.length - 1) : (index + direction + matches.length) % matches.length;
		if (matches[next]) this.choose(matches[next]);
	}

	async apply() {
		if (!this.ready || !this.spec) return false;
		this.saving = true;
		this.error = undefined;
		try {
			await selectTheme(this.spec);
			return true;
		} catch (failure) {
			this.error = failure instanceof Error ? failure.message : 'Could not save theme.';
			return false;
		} finally {
			this.saving = false;
		}
	}

	end() {
		this.#preview.end();
	}
}

export const themeBar = $state<{ choice?: ThemeChoice }>({});

export function previewThemes() {
	themeBar.choice ??= new ThemeChoice();
}

export function closeThemeBar() {
	themeBar.choice?.end();
	themeBar.choice = undefined;
}
