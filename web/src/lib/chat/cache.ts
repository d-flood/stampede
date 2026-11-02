import { ago } from '#lib/chat/sessions.ts';
import type { Cache } from '#lib/device.svelte.ts';
import { createSubscriber } from 'svelte/reactivity';

export type Warmth = 'warm' | Cache['then'];

const tick = createSubscriber((update) => {
	const timer = setInterval(update, 60_000);
	return () => clearInterval(timer);
});

export function warmth(cache: Cache): Warmth {
	tick();
	const now = Date.now();
	if (now < cache.expires) return 'warm';
	return cache.settles !== undefined && now >= cache.settles ? 'cold' : cache.then;
}

export function cold(warmth: Warmth | undefined) {
	return warmth === 'cold' || warmth === 'probably-cold';
}

export function describe(cache: Cache) {
	switch (warmth(cache)) {
		case 'warm':
			return cache.then === 'probably-warm' ? 'Cache warm' : `Cache warm · expires ${ago(cache.expires)}`;
		case 'probably-warm':
			return 'Cache probably warm';
		case 'probably-cold':
			return `Cache probably cold · expired ${ago(cache.expires)}`;
		case 'cold':
			return `Cache cold · expired ${ago(cache.expires)}`;
	}
}
