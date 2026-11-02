export type SpinnerStyle = 'custom' | 'standard';

const key = 'stampede.spinners';
export const spinners = $state<{ style: SpinnerStyle }>({ style: stored() });

function stored(): SpinnerStyle {
	try {
		return localStorage.getItem(key) === 'standard' ? 'standard' : 'custom';
	} catch {
		return 'custom';
	}
}

export function selectSpinners(style: SpinnerStyle) {
	spinners.style = style;
	try {
		if (style === 'custom') localStorage.removeItem(key);
		else localStorage.setItem(key, style);
	} catch {}
}
