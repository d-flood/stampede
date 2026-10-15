import { MediaQuery } from 'svelte/reactivity';

export type Form = 'phone' | 'desktop';

const phone = new MediaQuery('(pointer: coarse) and (max-width: 767px)');

export function form(): Form {
	return phone.current ? 'phone' : 'desktop';
}
