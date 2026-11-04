import { brief } from '#lib/chat/brief.ts';
import { count } from '#lib/chat/count.ts';
import { commands } from '#lib/chat/shells.ts';
import type { Item } from '#lib/device.svelte.ts';
import { expect, test } from 'vitest';

test('token labels keep the thousand and million boundaries readable', () => {
	expect([999, 1000, 999_499, 999_500, 1_050_000].map(count)).toEqual(['999', '1k', '999k', '1M', '1.1M']);
});

test('command summaries shorten private home paths and preserve command arguments', () => {
	expect(brief('cat /home/alice/project/notes.txt')).toBe('cat ~/project/notes.txt');
	expect(brief('cat /Users/alice/project/notes.txt')).toBe('cat ~/project/notes.txt');
	expect(brief('printf "hello world"')).toBe('printf "hello world"');
});

test('running-shell sources retain nested ownership and stable item paths', () => {
	const command: Item = { kind: 'command', command: 'pnpm dev', status: 'running', output: '' };
	const items: Item[] = [{ kind: 'text', text: 'Working', streaming: false, phase: 'commentary' }, command,
		{ kind: 'subagent', id: 'child', name: 'Builder', task: 'Build', status: 'done', items: [command] }];
	expect(commands(items)).toEqual([{ id: '/1', owner: 'Main', item: command }, { id: '/2/0', owner: 'Builder', item: command }]);
});
