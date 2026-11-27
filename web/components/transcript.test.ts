import MessageNav from '#lib/chat/MessageNav.svelte';
import Transcript from '#lib/chat/Transcript.svelte';
import type { Item } from '#lib/device.svelte.ts';
import { expect, onTestFinished, test, vi } from 'vitest';
import { page } from 'vitest/browser';
import { render } from 'vitest-browser-svelte';

const summaries = (container: HTMLElement) => [...container.querySelectorAll<HTMLButtonElement>('.calls .summary')];

test('messages stay visible while Activity groups update and retain their open state', async () => {
	const items: Item[] = [
		{ kind: 'text', text: 'I’ll read the files.', phase: 'commentary', streaming: false },
		{ kind: 'tools', calls: [{ kind: 'read', name: 'Read', detail: 'notes.txt' }] }
	];
	const view = await render(Transcript, { host: undefined, items, onopen: vi.fn(), onattend: vi.fn(), ondiff: vi.fn(), oncanvas: vi.fn() });
	await expect.element(view.getByText('I’ll read the files.', { exact: true })).toBeVisible();
	expect(summaries(view.container).map((element) => element.textContent)).toEqual(['Read 1 file']);
	expect(getComputedStyle(summaries(view.container)[0]).fontFamily).toContain('Fira Sans');
	expect(view.container.querySelector('.rail')).toBeNull();
	items[1] = { kind: 'tools', calls: [{ kind: 'read', name: 'Read', detail: 'notes.txt' }, { kind: 'search', name: 'Grep', detail: 'needle' }] };
	items.push({ kind: 'text', text: 'I found the match.', phase: 'commentary', streaming: false }, { kind: 'command', command: 'wc -l notes.txt', status: 'running', output: '' });
	await view.rerender({ items: [...items], activity: 'Running wc' });
	expect(summaries(view.container).map((element) => element.textContent)).toEqual(['Read 1 file, searched 1 time', 'Ran 1 command · Running wc']);
	await expect.element(view.getByText('I found the match.', { exact: true })).toBeVisible();
	await page.elementLocator(summaries(view.container)[1]).click();
	items[3] = { kind: 'command', command: 'wc -l notes.txt', status: 'ok', output: '3 notes.txt' };
	items.push({ kind: 'text', text: 'The file has three lines.', phase: 'final', streaming: false });
	await view.rerender({ items: [...items], activity: null });
	expect(summaries(view.container).map((element) => element.getAttribute('aria-expanded'))).toEqual(['false', 'true']);
	await expect.element(view.getByText('3 notes.txt', { exact: true })).toBeVisible();
	await expect.element(view.getByText('The file has three lines.', { exact: true })).toBeVisible();
	await page.elementLocator(summaries(view.container)[1]).click();
	await expect.poll(() => summaries(view.container)[0].getBoundingClientRect().height).toBeGreaterThanOrEqual(matchMedia('(pointer: coarse)').matches ? 48 : 24);
	await expect.poll(() => view.container.scrollWidth <= view.container.clientWidth).toBe(true);
});

test('Activity summaries count every action in order and keep failures quiet until expanded', async () => {
	const ondiff = vi.fn();
	const onexpand = vi.fn();
	const items: Item[] = [
		{ kind: 'text', text: 'Fixing the parser.', phase: 'commentary', streaming: false },
		{ kind: 'thinking', seconds: 2 },
		{ kind: 'tools', calls: [{ kind: 'read', name: 'Read', detail: 'src/parse.ts' }, { kind: 'read', name: 'Read', detail: 'src/lex.ts' }] },
		{ kind: 'edit', path: 'src/parse.ts', diff: '', added: 3, removed: 1 },
		{ kind: 'tools', calls: [{ kind: 'read', name: 'Read', detail: 'README.md' }] },
		{ kind: 'edit', path: 'src/parse.ts', diff: '', added: 2, removed: 0 },
		{ kind: 'command', command: 'pnpm lint', status: 'ok', output: 'clean' },
		{ kind: 'command', command: 'pnpm test parse', status: 'failed', output: '', elided: true }
	];
	const view = await render(Transcript, { host: undefined, items, onexpand, onopen: vi.fn(), onattend: vi.fn(), ondiff, oncanvas: vi.fn() });
	const [summary] = summaries(view.container);
	expect(summary.textContent).toBe('Edited 1 file, ran 2 commands, read 3 files, 1 failed');
	const segments = [...summary.querySelectorAll('.segment')];
	await expect.poll(() => getComputedStyle(segments.at(-1)!).color).toBe(getComputedStyle(segments[0]).color);

	const chips = view.container.querySelectorAll('.chip');
	expect([...chips].map((chip) => chip.textContent)).toEqual(['parse.ts+5−1']);
	await page.elementLocator(chips[0]).click();
	expect(ondiff).toHaveBeenCalledWith('src/parse.ts');

	expect(view.container.querySelector('.rail')).toBeNull();
	expect(view.container.textContent).not.toContain('pnpm test parse');
	await page.elementLocator(summary).click();
	expect(summary.getAttribute('aria-expanded')).toBe('true');
	expect(view.container.querySelectorAll('.chip')).toHaveLength(0);
	expect(view.container.querySelectorAll('.rail > .step')).toHaveLength(8);
	expect(view.container.querySelectorAll('.rail details')).toHaveLength(4);
	const failed = view.container.querySelector('.rail details.failed')!;
	expect(failed.hasAttribute('open')).toBe(false);
	const probe = view.container.appendChild(document.createElement('span'));
	probe.style.color = 'var(--ansi-1)';
	await expect.poll(() => getComputedStyle(view.container.querySelector('[data-dot="failed"] > .marker')!).color).toBe(getComputedStyle(probe).color);
	probe.remove();
	await page.elementLocator(failed.querySelector('summary')!).click();
	expect(failed.hasAttribute('open')).toBe(true);
	await expect.poll(() => onexpand.mock.calls).toEqual([[7]]);
	await expect.poll(() => view.container.scrollWidth <= view.container.clientWidth).toBe(true);
});

test('opening an elided command requests its original item index', async () => {
	const onexpand = vi.fn();
	const items: Item[] = [{ kind: 'text', text: 'Output follows.', phase: 'final', streaming: false }, { kind: 'command', command: 'cat notes.txt', status: 'ok', output: '', elided: true }];
	const view = await render(Transcript, { host: undefined, items, onexpand, onopen: vi.fn(), onattend: vi.fn(), ondiff: vi.fn(), oncanvas: vi.fn() });
	await page.elementLocator(summaries(view.container)[0]).click();
	await expect.poll(() => onexpand.mock.calls).toEqual([[1]]);
	await view.rerender({ items: [items[0], { kind: 'command', command: 'cat notes.txt', status: 'ok', output: 'Full command output' }] });
	await expect.element(view.getByText('Full command output', { exact: true })).toBeVisible();
});

test('tool arguments expand faithfully, copy their source, and retain expansion as Activity updates', async () => {
	const detail = String.raw`{"id":9007199254740993,"enabled":true,"nested":{"tags":["a","b"],"empty":{}},"text":"a \"quote\"; \\ path","enabled":false}`;
	const formatted = String.raw`{
  "id": 9007199254740993,
  "enabled": true,
  "nested": {
    "tags": [
      "a",
      "b"
    ],
    "empty": {}
  },
  "text": "a \"quote\"; \\ path",
  "enabled": false
}`;
	const plain = 'Arguments without JSON\n<img src=x onerror=alert(1)>';
	const calls = [{ kind: 'mcp', name: 'mcp__documents__read_document', detail }, { kind: 'other', name: 'Custom', detail: plain }] as const;
	const view = await render(Transcript, { host: undefined, items: [{ kind: 'tools', calls: [...calls] }], onopen: vi.fn(), onattend: vi.fn(), ondiff: vi.fn(), oncanvas: vi.fn() });
	const tools = () => [...view.container.querySelectorAll<HTMLDetailsElement>('.tool')];
	await page.elementLocator(summaries(view.container)[0]).click();
	expect(tools().map((tool) => tool.open)).toEqual([false, false]);
	expect(view.container.querySelector('pre')).toBeNull();
	await page.elementLocator(tools()[0].querySelector('summary')!).click();
	expect(tools()[0].querySelector('pre')?.textContent).toBe(formatted);
	await expect.poll(() => tools()[0].querySelectorAll('.wrap > span[class]').length).toBeGreaterThan(0);
	await page.elementLocator(tools()[0]).getByRole('button', { name: 'Copy original', exact: true }).click();
	expect(await navigator.clipboard.readText()).toBe(detail);
	await page.elementLocator(tools()[0]).getByRole('button', { name: 'Show original', exact: true }).click();
	expect(tools()[0].querySelector('pre')?.textContent).toBe(detail);
	await page.elementLocator(tools()[0]).getByRole('button', { name: 'Show formatted', exact: true }).click();
	await page.elementLocator(summaries(view.container)[0]).click();
	await page.elementLocator(summaries(view.container)[0]).click();
	expect(tools().map((tool) => tool.open)).toEqual([true, false]);
	await view.rerender({ items: [{ kind: 'tools', calls: [...calls, { kind: 'read', name: 'Read', detail: 'notes.txt' }] }] });
	expect(tools()[0].open).toBe(true);
	expect(tools()[0].querySelector('pre')?.textContent).toBe(formatted);
	await page.elementLocator(tools()[1].querySelector('summary')!).click();
	expect(tools()[1].querySelector('pre')?.textContent).toBe(plain);
	expect(tools()[1].querySelector('img')).toBeNull();
	expect(tools()[1].querySelectorAll('.wrap > span[class]')).toHaveLength(0);
	await expect.poll(() => view.container.scrollWidth <= view.container.clientWidth).toBe(true);
});

test('turns are spaced further apart than the entries within them', async () => {
	const items: Item[] = [
		{ kind: 'user', text: 'Debounce the watcher.', images: 0 },
		{ kind: 'text', text: 'I’ll map the consumers first.', phase: 'commentary', streaming: false },
		{ kind: 'tools', calls: [{ kind: 'read', name: 'Read', detail: 'watch.rs' }] },
		{ kind: 'text', text: 'The watcher is the place.', phase: 'commentary', streaming: false },
		{ kind: 'text', text: 'Done.', phase: 'final', streaming: false },
		{ kind: 'user', text: 'Check phone width too.', images: 0 },
		{ kind: 'text', text: 'Capturing it now.', phase: 'commentary', streaming: false }
	];
	const view = await render(Transcript, { host: undefined, items, onopen: vi.fn(), onattend: vi.fn(), ondiff: vi.fn(), oncanvas: vi.fn() });
	const blocks = [...view.container.querySelectorAll('.block')];
	const gaps = () => blocks.slice(1).map((block, index) => Math.round(block.getBoundingClientRect().top - blocks[index].getBoundingClientRect().bottom));
	const between = matchMedia('(pointer: coarse) and (max-width: 767px)').matches ? 32 : 44;
	expect(view.container.querySelectorAll('.turn')).toHaveLength(2);
	await expect.poll(gaps).toEqual([20, 8, 14, 22, between, 20]);
	expect(between).toBeGreaterThan(Math.max(...gaps().filter((gap) => gap !== between)));
});

test('a long Chat renders its newest turns first, then every older turn in order', async () => {
	const items = Array.from({ length: 40 }, (_, index): Item[] => [
		{ kind: 'user', text: `Prompt ${index + 1}`, images: 0 },
		{ kind: 'text', text: `Reply ${index + 1}`, phase: 'final', streaming: false }
	]).flat();
	const view = await render(Transcript, { host: undefined, items, onopen: vi.fn(), onattend: vi.fn(), ondiff: vi.fn(), oncanvas: vi.fn() });
	const prompts = () => [...view.container.querySelectorAll('[aria-label^="User message"]')].map((node) => node.getAttribute('aria-label'));
	expect(prompts().length).toBeLessThan(40);
	expect(prompts().at(-1)).toBe('User message 40');
	await expect.poll(prompts).toEqual(Array.from({ length: 40 }, (_, index) => `User message ${index + 1}`));
});

test('jumping to an unrendered turn renders it, then scrolls it into view', async () => {
	vi.stubGlobal('requestIdleCallback', () => 0);
	vi.stubGlobal('cancelIdleCallback', () => {});
	onTestFinished(() => { vi.unstubAllGlobals(); });
	const items = Array.from({ length: 40 }, (_, index): Item[] => [
		{ kind: 'user', text: `Prompt ${index + 1}`, images: 0 },
		{ kind: 'text', text: `Reply ${index + 1}`, phase: 'final', streaming: false }
	]).flat();
	const view = await render(Transcript, { host: undefined, items, ...handlers });
	const scroller = view.container;
	scroller.style.cssText = 'height: 400px; overflow: auto';
	scroller.scrollTop = 100;
	await render(MessageNav, { items, scroller, reveal: view.component.reveal });
	expect(scroller.querySelector('[data-item="0"]')).toBeNull();
	await page.getByRole('button', { name: /^(Message \d+ of 40|40 user messages)/ }).click();
	await page.getByRole('menuitemcheckbox', { name: '1. Prompt 1', exact: true }).click();
	const target = scroller.querySelector('[data-item="0"]')!;
	expect(scroller.querySelectorAll('[aria-label^="User message"]')).toHaveLength(40);
	await expect.poll(() => target.getBoundingClientRect().top).toBeGreaterThanOrEqual(scroller.getBoundingClientRect().top - 1);
	expect(target.getBoundingClientRect().top).toBeLessThan(scroller.getBoundingClientRect().bottom - target.getBoundingClientRect().height);
});

test('a counted image that could not be cached renders as an unavailable tile', async () => {
	const view = await render(Transcript, { host: undefined, items: [{ kind: 'user', text: '', images: 2, imageUrls: ['data:image/gif;base64,R0lGODlhAQABAAAAACw='] }], ...handlers });
	const images = view.container.querySelector('[aria-label="Attached images"]')!;
	expect(images.querySelectorAll('img')).toHaveLength(1);
	expect([...images.querySelectorAll('.unavailable')].map((tile) => tile.textContent)).toEqual(['Image unavailable']);
	expect(images.querySelector('.unavailable')!.closest('a')).toBeNull();
});

test('the prompt’s ⋯ menu opens Rewind without reserving layout space', async () => {
	const onrewind = vi.fn();
	const items: Item[] = [
		{ kind: 'text', text: 'Earlier context.', phase: 'final', streaming: false },
		{ kind: 'user', text: 'Short prompt.', images: 0 },
		{ kind: 'user', text: Array.from({ length: 20 }, (_, index) => `Line ${index + 1}`).join('\n'), images: 0 },
		{ kind: 'user', text: '/create-epic', template: 'create-epic', images: 0 }
	];
	const view = await render(Transcript, { host: undefined, items, onrewind, onopen: vi.fn(), onattend: vi.fn(), ondiff: vi.fn(), oncanvas: vi.fn() });
	const templated = view.getByRole('region', { name: 'User message 3', exact: true }).element();
	expect(templated.querySelector('.template')!.getBoundingClientRect().bottom).toBeLessThan(templated.querySelector('.user')!.getBoundingClientRect().bottom);
	const message = view.getByRole('region', { name: 'User message 1', exact: true });
	const bubble = message.element().querySelector('.user')!;
	const height = () => message.element().getBoundingClientRect().height;
	expect(height()).toBeCloseTo(bubble.getBoundingClientRect().height, 0);
	const resting = height();
	const more = view.getByRole('button', { name: 'Actions for message 1', exact: true });
	await message.hover();
	await expect.poll(() => getComputedStyle(more.element()).opacity).toBe('1');
	expect(height()).toBe(resting);
	(more.element() as HTMLElement).focus();
	expect(height()).toBe(resting);
	await more.click();
	await page.getByRole('menuitem', { name: 'Rewind…', exact: true }).click();
	expect(onrewind).toHaveBeenCalledWith(1);

	const long = view.getByRole('region', { name: 'User message 2', exact: true });
	await long.element().querySelector<HTMLElement>('summary')!.click();
	await expect.element(page.elementLocator(long.element().querySelector('.body')!)).toBeVisible();
	await view.getByRole('button', { name: 'Actions for message 2', exact: true }).click();
	await page.getByRole('menuitem', { name: 'Rewind…', exact: true }).click();
	expect(onrewind).toHaveBeenLastCalledWith(2);
});

const script = `S=/home/david/.local/share/app/scratchpad; cd "$S"
python3 - "$S/trial.mjs" <<'EOF'
import sys
print(open(sys.argv[1]).read().count("login"))
EOF
${Array.from({ length: 40 }, (_, index) => `curl -sf -o /dev/null http://127.0.0.1:44${index}/ && echo "ready ${index}"`).join('\n')}`;
const handlers = { onopen: vi.fn(), onattend: vi.fn(), ondiff: vi.fn(), oncanvas: vi.fn() };
const highlighted = (element: Element) => element.querySelectorAll('.syntax span[class]').length;

test('the live Activity reveals the current command or tool arguments on demand and follows later ones', async () => {
	const command = `cd web && rg -n 'a|b' src | sort`;
	const formatted = `cd web &&\n  rg -n 'a|b' src |\n  sort`;
	const items: Item[] = [{ kind: 'command', command, status: 'running', output: '', startedAt: Date.now() }];
	const view = await render(Transcript, { host: undefined, ...handlers, items: [...items], activity: `Running ${command}` });
	const live = () => view.container.querySelector<HTMLButtonElement>('.calls button.live')!;
	expect(view.container.querySelector('.current-command')).toBeNull();
	await page.elementLocator(live()).click();
	const current = view.container.querySelector('.current-command')!;
	await expect.poll(() => current.querySelector('pre')?.textContent).toBe(formatted);
	await expect.poll(() => highlighted(current)).toBeGreaterThan(0);
	await view.getByRole('button', { name: 'Copy original', exact: true }).click();
	expect(await navigator.clipboard.readText()).toBe(command);
	await view.getByRole('button', { name: 'Show original', exact: true }).click();
	expect(current.querySelector('pre')?.textContent).toBe(command);
	await view.getByRole('button', { name: 'Show formatted', exact: true }).click();
	items[0] = { ...items[0], status: 'ok', output: 'a.ts' } as Item;
	items.push({ kind: 'command', command: 'sleep 2', status: 'running', output: '', startedAt: Date.now() });
	await view.rerender({ items: [...items], activity: 'Running sleep 2' });
	expect(view.container.querySelector('.current-command pre')?.textContent).toBe('sleep 2');
	await page.elementLocator(summaries(view.container)[0]).click();
	const completed = view.container.querySelector<HTMLDetailsElement>('.rail details')!;
	expect(completed.open).toBe(true);
	expect(completed.querySelector('.command-code pre')?.textContent).toBe(formatted);
	await expect.element(view.getByText('a.ts', { exact: true })).toBeVisible();
	items.push({ kind: 'tools', calls: [{ kind: 'mcp', name: 'mcp__docs__read', detail: '{"id":1}' }] });
	await view.rerender({ items: [...items], activity: 'Using mcp__docs__read' });
	expect(view.container.querySelector('.current-tool pre')?.textContent).toBe('{\n  "id": 1\n}');
	expect(view.container.querySelector('.rail .tool')).toBeNull();
	await page.elementLocator(live()).click();
	expect(view.container.querySelector('.current-tool')).toBeNull();
	expect(view.container.querySelector('.rail .tool')).not.toBeNull();
	await expect.poll(() => view.container.scrollWidth <= view.container.clientWidth).toBe(true);
});

test('every summary segment stays readable beside a long Activity on a narrow screen', async () => {
	const { innerWidth: width, innerHeight: height } = window;
	onTestFinished(() => page.viewport(width, height));
	await page.viewport(360, height);
	const items: Item[] = [
		{ kind: 'tools', calls: [{ kind: 'read', name: 'Read', detail: 'a.ts' }, { kind: 'search', name: 'Grep', detail: 'b' }, { kind: 'web', name: 'WebSearch', detail: 'c' }, { kind: 'other', name: 'Custom', detail: '{}' }] },
		{ kind: 'command', command: 'pnpm lint', status: 'failed', output: '' },
		{ kind: 'command', command: script, status: 'running', output: '' }
	];
	const view = await render(Transcript, { host: undefined, ...handlers, items, activity: `Running ${script}` });
	const summary = summaries(view.container)[0];
	const live = view.container.querySelector<HTMLElement>('.calls .live')!;
	await expect.poll(() => summary.getBoundingClientRect().right).toBeLessThanOrEqual(360);
	const row = summary.parentElement!.getBoundingClientRect();
	for (const segment of summary.querySelectorAll('.segment')) {
		const box = segment.getBoundingClientRect();
		expect(box.left).toBeGreaterThanOrEqual(row.left);
		expect(box.right).toBeLessThanOrEqual(row.right);
		expect(box.bottom).toBeLessThanOrEqual(row.bottom);
	}
	expect([...summary.querySelectorAll('.segment')].at(-1)!.textContent).toBe('1 failed');
	expect(live.querySelector('.label')!.getBoundingClientRect().height).toBeLessThan(summary.querySelector('.segments')!.getBoundingClientRect().height);
	expect(live.getBoundingClientRect().right).toBeLessThanOrEqual(row.right);
});

test('only the label of a command in this group is highlighted', async () => {
	const items: Item[] = [{ kind: 'command', command: 'cargo test', status: 'running', output: '' }];
	const view = await render(Transcript, { host: undefined, ...handlers, items, activity: 'Running the tests' });
	const live = () => view.container.querySelector('.calls .live')!;
	expect(live().textContent).toBe(' · Running the tests');
	expect(live().querySelector('.syntax')).toBeNull();
	await view.rerender({ ...handlers, items, activity: 'Thinking…' });
	expect(live().textContent).toBe(' · Thinking…');
	expect(live().querySelector('.syntax')).toBeNull();
	await view.rerender({ ...handlers, items, activity: 'Running cargo test' });
	await expect.poll(() => highlighted(live())).toBeGreaterThan(0);
	expect(live().textContent).toBe(' · Running cargo test');
});

test('approval reviews stay in Activity, stay quiet when approved, and surface denials', async () => {
	const items: Item[] = [
		{ kind: 'tools', calls: [{ kind: 'approval', name: 'approved', detail: 'cargo test', rationale: 'Runs the workspace test suite.' }] },
		{ kind: 'command', command: 'cargo test', status: 'ok', output: 'ok' },
		{ kind: 'tools', calls: [{ kind: 'approval', name: 'denied', detail: 'touch /elsewhere', rationale: 'Writes outside the workspace.' }] }
	];
	const view = await render(Transcript, { host: undefined, ...handlers, items });
	expect(summaries(view.container).map((element) => element.textContent)).toEqual(['Ran 1 command, 1 approval denied']);
	await page.elementLocator(summaries(view.container)[0]).click();
	const approvals = [...view.container.querySelectorAll<HTMLDetailsElement>('.rail .approval')];
	expect(approvals).toHaveLength(2);
	expect(approvals[0].classList.contains('blocked')).toBe(false);
	expect(approvals[1].classList.contains('blocked')).toBe(true);
	await page.elementLocator(approvals[1].querySelector('summary')!).click();
	await expect.element(view.getByText('Writes outside the workspace.', { exact: true })).toBeVisible();
	await view.rerender({ ...handlers, items: [{ kind: 'tools', calls: [{ kind: 'approval', name: 'approved', detail: 'cargo test' }] }] });
	expect(summaries(view.container).map((element) => element.textContent)).toEqual(['Reviewed 1 approval']);
	await view.rerender({ ...handlers, items: [{ kind: 'command', command: 'cargo test', status: 'running', output: '' }, { kind: 'tools', calls: [{ kind: 'approval', name: 'approved', detail: 'cargo test' }] }], activity: 'Running cargo test' });
	await page.elementLocator(view.container.querySelector('.calls button.live')!).click();
	await expect.poll(() => view.container.querySelector('.current-command pre')?.textContent).toBe('cargo test');
	await expect.poll(() => view.container.scrollWidth <= view.container.clientWidth).toBe(true);
});
