import type { ServerMessage } from '#lib/device.svelte.ts';
import { expect, test } from './fixtures.ts';
import type { Locator, Page, WebSocketRoute } from '@playwright/test';
import { mkdirSync, writeFileSync } from 'node:fs';
import { newTab, openProject, screen, showSidebar } from './shell.ts';

declare global {
	interface Window {
		clicked?: number;
		loaded?: { at: number; data: string };
		received?: number;
		shown?: Promise<number>;
	}
}

const runs = 5;
const files = ['one.ts', 'two.ts', 'three.ts', 'four.ts', 'five.ts'];

function fixture(name: string) {
	const dir = `${process.env.STAMPEDE_E2E_ROOT}/projects/${name}`;
	mkdirSync(dir, { recursive: true });
	const step = (n: number) => `export function step${n}(input: number): number {\n\treturn input * ${n} + ${n};\n}\n`;
	const source = Array.from({ length: 50 }, (_, n) => step(n)).join('\n');
	for (const file of files) writeFileSync(`${dir}/${file}`, source);
	return name;
}

function longSession(session: string, turns: number) {
	const entries: object[] = [];
	const uuid = (index: number) => `${session.slice(0, 8)}-0000-4000-8000-${String(index).padStart(12, '0')}`;
	const push = (role: 'user' | 'assistant', content: unknown) =>
		entries.push({ type: role, uuid: uuid(entries.length), parentUuid: entries.length ? uuid(entries.length - 1) : null, sessionId: session, message: { role, content } });
	const code = (seed: number) => Array.from({ length: 12 }, (_, n) => `const value${n} = compute(${n}, ${seed});`).join('\n');
	for (let turn = 0; turn < turns; turn++) {
		push('user', `Turn ${turn}: handle case ${turn} in the \`parser\`.`);
		push('assistant', [{ type: 'tool_use', id: `read-${turn}`, name: 'Read', input: { file_path: `/project/parser${turn % 5}.ts` } }]);
		push('user', [{ type: 'tool_result', tool_use_id: `read-${turn}`, content: code(turn) }]);
		push('assistant', [{ type: 'tool_use', id: `test-${turn}`, name: 'Bash', input: { command: 'pnpm test parser' } }]);
		push('user', [{ type: 'tool_result', tool_use_id: `test-${turn}`, content: ' ✓ parser\n ✓ lexer' }]);
		push('assistant', [{ type: 'text', text: `## Case ${turn}\n\nHandled **case ${turn}** in \`parse()\`.\n\n- Kept the API\n- Added tests\n\n\`\`\`ts\n${code(turn)}\n\`\`\`` }]);
	}
	return entries.map((entry) => JSON.stringify(entry)).join('\n');
}

function mark(page: Page, name: string, after = 0) {
	return page.evaluate(
		([name, after]) =>
			new Promise<number>((resolve) => {
				new PerformanceObserver((list, observer) => {
					const mark = list.getEntriesByName(name).find((mark) => mark.startTime > after);
					if (!mark) return;
					observer.disconnect();
					resolve(mark.startTime);
				}).observe({ type: 'mark', buffered: true });
			}),
		[name, after] as const
	);
}

async function newTerminal(page: Page) {
	const tabs = page.getByRole('tablist', { name: 'Tabs' }).getByRole('tab');
	const count = await tabs.count();
	await newTab(page, 'Terminal');
	await expect(tabs).toHaveCount(count + 1);
	const id = (await tabs.last().getAttribute('data-tab'))!;
	await mark(page, `painted:${id}`);
	await expect.poll(() => screen(page), { timeout: 15_000 }).toMatch(/\S/);
	return id;
}

async function measure(page: Page, target: Locator, tab: Locator) {
	await page.evaluate(() =>
		addEventListener('click', ({ timeStamp }) => (window.clicked = timeStamp), { capture: true, once: true })
	);
	await target.click();
	const [clicked, id] = await Promise.all([page.evaluate(() => window.clicked ?? Infinity), tab.getAttribute('data-tab')]);
	return (await mark(page, `painted:${id}`, clicked)) - clicked;
}

function check(name: string, times: number[], budget: number) {
	const median = times.toSorted((a, b) => a - b)[Math.floor(times.length / 2)];
	console.log(`${test.info().project.name} ${name}: median ${median.toFixed(1)} ms of ${times.map((time) => time.toFixed(1))}`);
	expect.soft(median, `${name} median`).toBeLessThanOrEqual(budget);
}

test('switching Tab or Project paints within 50 ms, and opening a file within 100 ms', async ({ page }, { project }) => {
	const [one, two] = [fixture(`perf-${project.name}-one`), fixture(`perf-${project.name}-two`)];
	await openProject(page, two);
	const other = await newTerminal(page);
	await openProject(page, one);
	const terminals = [await newTerminal(page), await newTerminal(page)];
	const tab = (id: string) => page.locator(`[data-tab="${id}"]`);

	const tabSwitches = [];
	for (let run = 0; run < runs; run++) {
		const id = terminals[run % 2];
		tabSwitches.push(await measure(page, tab(id), tab(id)));
	}
	check('Tab switch', tabSwitches, 50);

	const projectSwitches = [];
	for (let run = 0; run < runs; run++) {
		const [name, id] = run % 2 ? [one, terminals[0]] : [two, other];
		const entry = page.getByRole('navigation', { name: 'Projects' }).getByRole('button', { name, exact: true });
		projectSwitches.push(await measure(page, entry, tab(id)));
	}
	check('Project switch', projectSwitches, 50);

	const opens = [];
	for (const file of files) {
		const sidebar = await showSidebar(page);
		// Find the Tab by CSS: a role query computes accessible names on the main thread being timed.
		opens.push(await measure(page, sidebar.getByRole('button', { name: file, exact: true }), page.locator('[role="tab"]', { hasText: file })));
	}
	check('Open file', opens, 100);
});

test('a long Chat paints within 300 ms of loading, switches back within 150 ms, and shows a new item within 80 ms', async ({ page }, { project }) => {
	let route: WebSocketRoute;
	await page.routeWebSocket('**/ws', (socket) => {
		route = socket;
		const server = socket.connectToServer();
		server.onMessage((data) => socket.send(data));
	});
	await page.addInitScript(() => {
		const Native = WebSocket;
		window.WebSocket = class extends Native {
			constructor(...args: ConstructorParameters<typeof WebSocket>) {
				super(...args);
				this.addEventListener('message', ({ data }) => {
					if (typeof data !== 'string') return;
					if (data.startsWith('{"type":"chat.items"')) window.loaded = { at: performance.now(), data };
					else if (data.startsWith('{"type":"chat.item"')) window.received = performance.now();
				});
			}
		};
	});
	const name = `long-chat-${project.name}`;
	const path = `${process.env.STAMPEDE_E2E_ROOT}/projects/${name}`;
	const sessions = `${process.env.STAMPEDE_E2E_ROOT}/claude/projects/${path.replace(/[^a-zA-Z0-9]/g, '-')}`;
	const session = '5e5510b0-1111-4111-8111-111111111111';
	mkdirSync(path, { recursive: true });
	mkdirSync(sessions, { recursive: true });
	writeFileSync(`${sessions}/${session}.jsonl`, longSession(session, 500));
	await openProject(page, name);
	const terminal = page.locator(`[data-tab="${await newTerminal(page)}"]`);
	await page.getByRole('button', { name: 'New Tab' }).click();
	await page.getByRole('group', { name: 'Sessions' }).getByRole('menuitem').filter({ hasText: 'Turn 0:' }).click();
	const id = (await page.getByRole('tab', { name: /^Turn 0:/ }).getAttribute('data-tab'))!;
	const painted = await mark(page, `painted:${id}`);
	const loaded = (await page.evaluate(() => window.loaded))!;
	check('Long Chat paint', [painted - loaded.at], 300);
	await expect(page.locator('.panel:not([hidden]) [data-user-message]')).toHaveCount(500, { timeout: 30_000 });

	const chat = page.locator(`[data-tab="${id}"]`);
	const switches = [];
	for (let run = 0; run < runs; run++) {
		await terminal.click();
		switches.push(await measure(page, chat, chat));
	}
	check('Long Chat switch', switches, 150);

	const count = JSON.parse(loaded.data).items.length;
	const additions = [];
	for (let run = 0; run < runs; run++) {
		const text = `New item ${run}`;
		await page.evaluate((text) => {
			const panel = document.querySelector('.panel:not([hidden])')!;
			window.shown = new Promise((resolve) => {
				const observer = new MutationObserver(() => {
					if (!panel.querySelector('.turn:last-child')?.textContent?.includes(text)) return;
					observer.disconnect();
					requestAnimationFrame(() => setTimeout(() => resolve(performance.now() - window.received!)));
				});
				observer.observe(panel, { subtree: true, childList: true, characterData: true });
			});
		}, text);
		route!.send(JSON.stringify({ type: 'chat.item', chatId: id, index: count + run, item: { kind: 'text', text, streaming: false, phase: 'commentary' } }));
		additions.push(await page.evaluate(() => window.shown!));
	}
	check('Long Chat new item', additions, 80);
});

for (const streaming of [true, false]) {
	test(`closing a Chat releases cached ${streaming ? 'streaming' : 'completed'} reply versions`, async ({ page }, { project }) => {
		let route: WebSocketRoute;
		let chatId: string;
		await page.routeWebSocket('**/ws', (socket) => {
			const server = socket.connectToServer();
			server.onMessage((data) => {
				if (typeof data !== 'string') return socket.send(data);
				const message: ServerMessage = JSON.parse(data);
				if (message.type === 'chat.created') { route = socket; chatId = message.chat.id; }
				socket.send(data);
			});
		});
		const name = `chat-memory-${project.name}`;
		mkdirSync(`${process.env.STAMPEDE_E2E_ROOT}/projects/${name}`);
		await openProject(page, name);
		await page.getByRole('button', { name: 'New Tab' }).click();
		await page.getByRole('menuitem', { name: 'Fake Claude', exact: true }).click();
		await expect(page.getByRole('region', { name: 'New Chat' })).toBeVisible();
		const cdp = await page.context().newCDPSession(page);
		const heap = async () => {
			await cdp.send('HeapProfiler.collectGarbage');
			return (await cdp.send('Runtime.getHeapUsage')).usedSize;
		};
		const before = await heap();
		const text = 'A paragraph with **formatting** and `code`.\n\n'.repeat(1000);
		route!.send(JSON.stringify({ type: 'chat.item', chatId: chatId!, index: 0, item: { kind: 'text', text, streaming, phase: 'commentary' } }));
		const reply = page.locator('.panel:visible .text');
		await expect(reply.locator('p')).toHaveCount(1000);
		for (let index = 0; index < 100; index++) {
			const delta = ` Snapshot ${index}.`;
			route!.send(JSON.stringify({ type: 'chat.append', chatId: chatId!, index: 0, text: delta }));
			await expect.poll(() => reply.evaluate((node, suffix) => node.textContent!.trimEnd().endsWith(suffix), delta.trimStart()), { intervals: [10] }).toBe(true);
		}
		await page.getByRole('button', { name: 'Close Fake Claude', exact: true }).click();
		await expect(page.getByRole('tab', { name: 'Fake Claude', exact: true })).toHaveCount(0);
		await expect(page.locator('.panel .text')).toHaveCount(0);
		const after = await heap();
		console.log(`${project.name} ${streaming ? 'streaming' : 'completed'} Chat retained ${(after - before) / 2**20} MiB after closing`);
		expect(after - before).toBeLessThan(8 << 20);
	});
}
