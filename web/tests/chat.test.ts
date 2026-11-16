import { brief } from '#lib/chat/brief.ts';
import { count } from '#lib/chat/count.ts';
import { formatCommand } from '#lib/chat/format-command.ts';
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

test('command formatting breaks shell operators while preserving arguments and uncertain syntax', () => {
	const cases = [
		['cd web && rg -n \'a|b; c && d\' src | sort || echo "failed"; pwd', 'cd web &&\n  rg -n \'a|b; c && d\' src |\n  sort ||\n  echo "failed";\npwd'],
		['build 2>&1 |& tee build.log & echo started', 'build 2>&1 |&\n  tee build.log &\necho started'],
		['printf a\\|b | cat >|result', 'printf a\\|b |\n  cat >|result'],
		['echo "$HOME" && echo done', 'echo "$HOME" &&\n  echo done'],
		['python3 -c \'print("a;b|c")\' | head', 'python3 -c \'print("a;b|c")\' |\n  head']
	];
	for (const [command, expected] of cases) expect(formatCommand(command)).toBe(expected);
	for (const command of [
		'pnpm -C web check',
		'echo "unfinished | cat',
		'echo a | cat \\',
		'echo a # comment | cat',
		'cat <<EOF | sort\na|b\nEOF',
		'cat <<EOF | sort',
		'if true; then echo yes; fi',
		'[[ a || b ]] && echo yes',
		'echo $(printf "a|b") | cat',
		'echo "$(printf "a|b")" | cat',
		'echo ${value:-a|b} | cat',
		'echo `printf a` | cat',
		"printf $'a\\\'|b' | cat",
		'(echo a; echo b) | cat'
	]) expect(formatCommand(command)).toBe(command);
});
