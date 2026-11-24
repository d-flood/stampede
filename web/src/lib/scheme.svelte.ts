export type Scheme = 'light' | 'dark' | 'system';

const key = 'stampede.color-scheme';
const system = matchMedia('(prefers-color-scheme: dark)');
let systemDark = $state(system.matches);
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
	return colorScheme.choice === 'system' ? systemDark : colorScheme.choice === 'dark';
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

// Effects that read colors from CSS run as soon as a listener returns, so one listener updates both.
system.addEventListener('change', () => {
	systemDark = system.matches;
	apply();
});
apply();
