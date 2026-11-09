import type { ClientMessage, Device } from '#lib/device.svelte.ts';
import { languageServer } from '#lib/editor/lsp-editor.ts';
import { LanguageServers } from '#lib/editor/lsp.svelte.ts';
import { EditorView } from '@codemirror/view';
import { expect, test } from 'vitest';

test('a visible linter retries startup when project files change', async () => {
	const sent: ClientMessage[] = [];
	const servers = new LanguageServers({ send: (message: ClientMessage) => sent.push(message) } as unknown as Device, () => '/project', () => {});
	servers.message({ type: 'lsp.switch', projectId: 'p', on: true });
	const view = new EditorView({ doc: 'import os\n', extensions: languageServer(servers, 'p', 'demo.py', true), parent: document.body });
	await Promise.resolve();
	servers.displayed(view, true);
	servers.message({ type: 'lsp.state', channel: 'lsp:p:python', state: 'starting' });
	const opens = () => sent.filter((message) => message.type === 'lsp.open').map((message) => 'channel' in message && message.channel);
	expect(opens()).toEqual(['lsp:p:python', 'lsp:p:ruff']);

	sent.length = 0;
	servers.message({ type: 'fs.changed', projectId: 'p', paths: ['pyproject.toml'] });
	expect(opens()).toEqual(['lsp:p:ruff']);

	sent.length = 0;
	servers.message({ type: 'lsp.state', channel: 'lsp:p:ruff', state: 'error' });
	servers.message({ type: 'fs.changed', projectId: 'p', paths: ['pyproject.toml'] });
	expect(opens()).toEqual([]);

	servers.message({ type: 'lsp.state', channel: 'lsp:p:ruff', state: 'off' });
	servers.displayed(view, false);
	sent.length = 0;
	servers.message({ type: 'fs.changed', projectId: 'p', paths: ['pyproject.toml'] });
	expect(opens()).toEqual([]);
	view.destroy();
});
