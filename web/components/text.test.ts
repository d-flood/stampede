import Text from '#lib/chat/Text.svelte';
import { expect, test, vi } from 'vitest';
import { render } from 'vitest-browser-svelte';

const item = (text: string) => ({ kind: 'text' as const, text, streaming: false, phase: 'final' as const });

test('tables keep short words whole and headings step down in size', async () => {
	const prose = 'A long explanation that wraps across several lines in a narrow column. '.repeat(3);
	const view = await render(Text, { item: item(`# One\n\n## Two\n\n### Three\n\nBody\n\n| Agent | Notes |\n| --- | --- |\n| Copilot | ${prose} |`) });
	const lines = (element: Element) => {
		const range = document.createRange();
		range.selectNodeContents(element);
		return range.getClientRects().length;
	};
	await expect.poll(() => lines(view.container.querySelector('th')!)).toBe(1);
	await expect.poll(() => lines(view.container.querySelector('td')!)).toBe(1);
	await expect.poll(() => {
		const sizes = [...view.container.querySelectorAll('h1, h2, h3, p')].map((element) => parseFloat(getComputedStyle(element).fontSize));
		return { descending: sizes.every((size, index) => index === 0 || sizes[index - 1] > size), unique: new Set(sizes).size };
	}).toEqual({ descending: true, unique: 4 });
});

test('table columns fit short text and scroll for long text', async () => {
	const row = (cells: string[]) => `| ${cells.join(' | ')} |`;
	const prose = 'A long explanation that wraps across several lines in a narrow column. '.repeat(3);
	const view = await render(Text, { item: item([
		row(['A', 'B', 'C']), row(Array(3).fill('---')), row(Array(3).fill('short text')),
		'', row(['Agent', 'First', 'Second', 'Third']), row(['---', '---:', '---', '---']), row(['Copilot', prose, prose, prose])
	].join('\n')) });
	const [short, long] = view.container.querySelectorAll<HTMLElement>('.table');
	await expect.poll(() => short.scrollWidth > short.clientWidth).toBe(false);
	await expect.poll(() => long.scrollWidth > long.clientWidth).toBe(true);
	const agent = long.querySelector('td')!;
	const notes = long.querySelectorAll('td')[1];
	await expect.poll(() => notes.getBoundingClientRect().width).toBeGreaterThan(200);
	await expect.poll(() => agent.getBoundingClientRect().width / notes.getBoundingClientRect().width).toBeLessThan(0.5);
	await expect.poll(() => getComputedStyle(long.querySelectorAll('th')[1]).textAlign).toBe('right');
});

test('code blocks and long inline code copy their text and clear feedback', async () => {
	const copy = vi.spyOn(navigator.clipboard, 'writeText').mockResolvedValue();
	const view = await render(Text, { item: item('Run `ls` or `pnpm install --frozen-lockfile`:\n\n```sh\necho one\necho two\n```') });
	vi.useFakeTimers({ toFake: ['setTimeout', 'clearTimeout'] });
	const buttons = view.getByRole('button', { name: 'Copy' });
	expect(buttons.elements()).toHaveLength(2);
	await buttons.first().click();
	expect(copy).toHaveBeenLastCalledWith('pnpm install --frozen-lockfile');
	await buttons.last().click();
	expect(copy).toHaveBeenLastCalledWith('echo one\necho two');
	await expect.element(buttons.last()).toHaveAttribute('data-copied');
	await vi.advanceTimersByTimeAsync(1500);
	await expect.element(buttons.last()).not.toHaveAttribute('data-copied');
});

test('inactive text defers new contents until shown', async () => {
	vi.useFakeTimers({ toFake: ['requestAnimationFrame', 'cancelAnimationFrame'] });
	const view = await render(Text, { item: { ...item('Initial reply.'), streaming: true }, active: true });
	await view.rerender({ active: false });
	await view.rerender({ item: item('Finished reply.') });
	await vi.advanceTimersByTimeAsync(32);
	expect(view.container.textContent?.trim()).toBe('Initial reply.');
	await view.rerender({ active: true });
	await expect.poll(() => view.container.textContent?.trim()).toBe('Finished reply.');
});

test('inline code follows the chat size and code blocks use four-column tabs', async () => {
	document.documentElement.style.setProperty('--code-size', '24px');
	try {
		const view = await render(Text, { item: item('Call `parse` here.\n\n```rust\nfn main() {\n\tlet x = 1;\n}\n```') });
		const paragraph = view.container.querySelector('p')!;
		const size = (element: Element) => parseFloat(getComputedStyle(element).fontSize);
		await expect.poll(() => size(paragraph.querySelector('code')!)).toBeCloseTo(0.85 * size(paragraph), 1);
		expect(size(view.container.querySelector('pre code')!)).toBe(24);
		expect(getComputedStyle(view.container.querySelector('pre')!).tabSize).toBe('4');
	} finally {
		document.documentElement.style.removeProperty('--code-size');
	}
});

test.runIf(matchMedia('(pointer: coarse) and (max-width: 767px)').matches)('phone tables scroll inside their wrapper without wrapping cells', async () => {
	const view = await render(Text, { item: item('| Check | Result | Notes |\n| --- | --- | --- |\n| `./check e2e changes.spec.ts` | 1 failed | The Changes list missed a rename after the debounce window closed |') });
	const wrapper = view.container.querySelector<HTMLElement>('.table')!;
	expect(wrapper.querySelector('table')).not.toBeNull();
	await expect.poll(() => wrapper.scrollWidth > wrapper.clientWidth).toBe(true);
	expect([...wrapper.querySelectorAll('th, td')].map((cell) => getComputedStyle(cell).whiteSpace)).toEqual(Array(6).fill('nowrap'));
	expect(document.documentElement.scrollWidth).toBeLessThanOrEqual(document.documentElement.clientWidth);
	expect(view.container.scrollWidth).toBeLessThanOrEqual(view.container.clientWidth);
});
