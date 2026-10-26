import type { Item, Request, ServerMessage } from '#lib/device.svelte.ts';
import { expect, test } from './fixtures.ts';
import type { Locator, WebSocketRoute } from '@playwright/test';
import { mkdirSync } from 'node:fs';
import { closeProject, openProject } from './shell.ts';

const patch = [
	'--- a/example.ts',
	'+++ b/example.ts',
	'@@ -1,3 +1,3 @@',
	'-export const value = "old";',
	'+export const value = "<img src=x onerror=alert(1)>";',
	'-/* removed comment',
	'+export const enabled = true;',
	' const number = 42;',
	'@@ -10,3 +10,3 @@',
	' /* shared comment',
	'- old text',
	'+ new text',
	'  */',
	''
].join('\n');

const items: Item[] = [
	{ kind: 'command', command: 'printf \'%s\\n\' "$HOME"', status: 'ok', output: 'plain output' },
	{ kind: 'tools', calls: [{ kind: 'other', name: 'example', detail: '{"enabled":true,"count":42}' }] },
	{ kind: 'edit', path: 'example.ts', diff: patch, added: 3, removed: 3 },
	{ kind: 'edit', path: 'notes.unknown', diff: '@@ -1 +1 @@\n-<old>\n+<new>\n', added: 1, removed: 1 },
	{ kind: 'command', command: 'pnpm -C web check', status: 'running', output: '' },
	{ kind: 'command', command: "sed -n '1,125p' web/e2e/chat-requests.spec.ts", status: 'ok', output: '' },
	{ kind: 'command', command: "ps -eo pid,ppid,comm | rg 'pnpm|node|zsh'", status: 'ok', output: '' },
	{ kind: 'command', command: 'cat web/e2e/search.spec.ts | head -42; git diff', status: 'ok', output: '' },
	{ kind: 'command', command: "python3 -c 'import json; print(json.dumps(1))' | head", status: 'ok', output: '' },
	{ kind: 'command', command: "python3 -I - <<'EOF'\nfor line in open('x'):\n    print(line)\nEOF\necho done", status: 'ok', output: '' }
];

test('chat highlights commands, tool arguments, and both sides of inline diffs', { tag: '@desktop' }, async ({ page }, { project, repeatEachIndex }) => {
	let route: WebSocketRoute;
	let chatId: string;
	const transcript = [...items];
	await page.routeWebSocket('**/ws', (socket) => {
		const server = socket.connectToServer();
		server.onMessage((data) => {
			if (typeof data !== 'string') return socket.send(data);
			const message: ServerMessage = JSON.parse(data);
			if (message.type === 'chat.created') {
				route = socket;
				chatId = message.chat.id;
			}
			if (message.type === 'chat.items' && message.chatId === chatId) {
				socket.send(JSON.stringify({ ...message, items: transcript }));
			} else socket.send(data);
		});
	});
	const name = `highlighting-${project.name}-${repeatEachIndex}`;
	mkdirSync(`${process.env.STAMPEDE_E2E_ROOT}/projects/${name}`, { recursive: true });
	await openProject(page, name);
	await page.getByRole('button', { name: 'New Tab' }).click();
	await page.getByRole('menuitem', { name: 'Fake Claude', exact: true }).click();
	const panel = page.getByRole('tabpanel').filter({ has: page.getByRole('textbox', { name: 'Composer' }) });
	const command = panel.locator('.transcript .command').first();
	await expect(command).toBeHidden();
	await panel.locator('.calls > summary').click();
	await expect(command).toHaveText('$ printf \'%s\\n\' "$HOME"');
	await colored(command.getByText("'%s\\n'", { exact: true }), '--ansi-2');
	transcript[0] = { kind: 'command', command: 'printf "updated"', status: 'ok', output: 'plain output' };
	route!.send(JSON.stringify({ type: 'chat.item', chatId: chatId!, index: 0, item: transcript[0] }));
	await colored(command.getByText('"updated"', { exact: true }), '--ansi-2');
	const pnpm = panel.locator('.transcript .command').filter({ hasText: 'pnpm -C' });
	await colored(pnpm.getByText('pnpm', { exact: true }), '--ansi-4');
	await colored(pnpm.getByText('-C', { exact: true }), '--ansi-3');
	const sed = panel.locator('.transcript .command').filter({ hasText: 'sed -n' });
	await colored(sed.getByText('sed', { exact: true }), '--ansi-4');
	await colored(sed.getByText("'1,125p'", { exact: true }), '--ansi-2');
	const pipeline = panel.locator('.transcript .command').filter({ hasText: 'ps -eo' });
	await colored(pipeline.getByText('ps', { exact: true }), '--ansi-4');
	await colored(pipeline.getByText('rg', { exact: true }), '--ansi-4');
	await colored(pipeline.getByText('|', { exact: true }), '--muted-fg');
	const chained = panel.locator('.transcript .command').filter({ hasText: 'cat web/e2e' });
	await colored(chained.getByText('head', { exact: true }), '--ansi-4');
	await colored(chained.getByText('git', { exact: true }), '--ansi-4');
	const inline = panel.locator('.transcript .command').filter({ hasText: 'import json' });
	await colored(inline.getByText('import', { exact: true }), '--ansi-5');
	await colored(inline.getByText('head', { exact: true }), '--ansi-4');
	const heredoc = panel.locator('.transcript .command').filter({ hasText: 'for line' });
	await colored(heredoc.getByText('for', { exact: true }), '--ansi-5');
	await colored(heredoc.getByText('echo', { exact: true }), '--ansi-4');
	await panel.getByText('Used example', { exact: true }).click();
	await colored(panel.getByRole('listitem').getByText('true', { exact: true }), '--ansi-3');

	const edit = panel.locator('.edit').filter({ hasText: 'example.ts' });
	const unknown = panel.locator('.edit').filter({ hasText: 'notes.unknown' });
	const added = edit.locator('.code-line.added').filter({ hasText: 'enabled' });
	await colored(added.getByText('const', { exact: true }), '--ansi-5');
	await colored(added.getByText('true', { exact: true }), '--ansi-3');
	await colored(edit.locator('.removed').getByText('"old"', { exact: true }), '--ansi-2');
	await colored(edit.locator('.added').getByText(' new text', { exact: true }), '--muted-fg');
	await expect(edit.locator('.code-line').first()).toHaveText('--- a/example.ts');
	await expect(edit.locator('img')).toHaveCount(0);
	await expect(edit).toContainText('+export const value = "<img src=x onerror=alert(1)>";');
	await expect(unknown.locator('.code-line.added')).toHaveText('+<new>');
	await expect(unknown.locator('.code-line span[class]')).toHaveCount(0);
	await expect.poll(() => added.evaluate((node) => getComputedStyle(node).backgroundColor)).not.toBe('rgba(0, 0, 0, 0)');
	await expect.poll(() => panel.evaluate((node) => node.scrollWidth <= node.clientWidth)).toBe(true);

	function request(request: Request) {
		route.send(JSON.stringify({ type: 'chat.request', chatId, request }));
	}
	request({ kind: 'edit', id: 'highlight-edit', path: 'example.ts', diff: patch });
	const approval = panel.getByRole('region', { name: 'Approval' });
	await colored(approval.locator('.added').getByText('true', { exact: true }), '--ansi-3');
	route!.send(JSON.stringify({ type: 'chat.resolved', chatId: chatId!, requestId: 'highlight-edit' }));
	request({ kind: 'command', id: 'highlight-command', words: [['printf', 'hello world', '$HOME']], suggestion: [], sessionPrefixes: true });
	await colored(approval.locator('pre').getByText("'hello world'", { exact: true }), '--ansi-2');
	route!.send(JSON.stringify({ type: 'chat.resolved', chatId: chatId!, requestId: 'highlight-command' }));
	request({ kind: 'command', id: 'highlight-python', words: [['python3', '-c', "print('hi')"]], suggestion: [], sessionPrefixes: true });
	await expect(approval.locator('pre')).toHaveText(`python3 -c "print('hi')"`);
	await colored(approval.locator('pre').getByText('print', { exact: true }), '--ansi-4');
	await closeProject(page, name);
});

async function colored(locator: Locator, token: string) {
	await expect(locator).toBeVisible();
	await expect.poll(() => locator.evaluate((node, token) => {
		const probe = document.body.appendChild(document.createElement('span'));
		probe.style.color = `var(${token})`;
		const color = getComputedStyle(probe).color;
		probe.remove();
		return getComputedStyle(node).color === color;
	}, token)).toBe(true);
}
