(() => {
	const targetOrigin = document.currentScript.dataset.parentOrigin;
	if (window.parent === window || !targetOrigin) return;
	const type = 'stampede.console';
	const post = (message) => window.parent.postMessage({ type, ...message }, targetOrigin);
	let pending = [];
	let dropped = 0;
	let timer;

	function format(value, depth = 0, seen = new WeakSet()) {
		try {
			if (typeof value === 'string') return value.slice(0, 2000);
			if (typeof value === 'bigint') return `${value}n`;
			if (value === null || typeof value !== 'object') return String(value);
			if (value instanceof Error) return (value.stack || `${value.name}: ${value.message}`).slice(0, 4000);
			if (seen.has(value)) return '[Circular]';
			if (depth >= 3) return Array.isArray(value) ? '[Array]' : '[Object]';
			seen.add(value);
			const keys = Object.keys(value).slice(0, 20);
			const fields = keys.map((key) => {
				const descriptor = Object.getOwnPropertyDescriptor(value, key);
				const text = descriptor && 'value' in descriptor ? format(descriptor.value, depth + 1, seen) : '[Getter]';
				return Array.isArray(value) ? text : `${key}: ${text}`;
			});
			if (Object.keys(value).length > keys.length) fields.push('…');
			seen.delete(value);
			return Array.isArray(value) ? `[${fields.join(', ')}]` : `{ ${fields.join(', ')} }`;
		} catch {
			return '[Uninspectable]';
		}
	}

	function flush() {
		timer = undefined;
		if (dropped) pending.push({ level: 'warn', text: `${dropped} console messages omitted`, time: Date.now() });
		post({ kind: 'entries', entries: pending });
		pending = [];
		dropped = 0;
	}

	function capture(level, args) {
		try {
			if (pending.length < 100) {
				pending.push({ level, text: args.slice(0, 20).map((value) => format(value)).join(' ').slice(0, 8000), time: Date.now() });
			} else dropped++;
			timer ??= setTimeout(flush, 50);
		} catch {}
	}

	for (const level of ['log', 'debug', 'info', 'warn', 'error', 'trace']) {
		const original = console[level];
		console[level] = function (...args) {
			Reflect.apply(original, console, args);
			capture(level, level === 'trace' ? [...args, new Error('Trace')] : args);
		};
	}
	const assert = console.assert;
	console.assert = function (condition, ...args) {
		Reflect.apply(assert, console, [condition, ...args]);
		if (!condition) capture('error', ['Assertion failed:', ...args]);
	};
	const clear = console.clear;
	console.clear = function () {
		Reflect.apply(clear, console, []);
		clearTimeout(timer);
		timer = undefined;
		pending = [];
		dropped = 0;
		post({ kind: 'clear' });
	};
	window.addEventListener('error', (event) => {
		if (event instanceof ErrorEvent) capture('error', [event.error || `${event.message} (${event.filename}:${event.lineno}:${event.colno})`]);
	});
	window.addEventListener('unhandledrejection', (event) => capture('error', ['Unhandled promise rejection:', event.reason]));
	window.addEventListener('pagehide', () => post({ kind: 'disconnected' }));
	post({ kind: 'ready' });
})();
