import { MediaQuery } from 'svelte/reactivity';

export type Scheme = 'light' | 'dark' | 'system';

const key = 'stampede.color-scheme';
const system = new MediaQuery('(prefers-color-scheme: dark)');
export const colorScheme = $state<{ choice: Scheme }>({ choice: stored() });

function stored(): Scheme {
	try {
		const choice = localStorage.getItem(key);
		return choice === 'light' || choice === 'dark' ? choice : 'system';
	} catch {
		return 'system';
	}
}

export function isDark(): boolean {
	return colorScheme.choice === 'system' ? system.current : colorScheme.choice === 'dark';
}

function apply() {
	document.documentElement.dataset.scheme = isDark() ? 'dark' : 'light';
}

export function selectScheme(choice: Scheme) {
	colorScheme.choice = choice;
	apply();
	try {
		if (choice === 'system') localStorage.removeItem(key);
		else localStorage.setItem(key, choice);
	} catch {}
}

matchMedia('(prefers-color-scheme: dark)').addEventListener('change', apply);
apply();
