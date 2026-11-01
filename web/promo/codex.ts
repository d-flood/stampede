import { readFileSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { patch } from './chat.ts';

const recorded = fileURLToPath(new URL('../../crates/stampede/tests/fixtures/chats/codex/approval.recorded.jsonl', import.meta.url));
const cwd = '/tmp/stampede-chat';

type Entry = Record<string, any>;

export class Codex {
	entries: Entry[] = [];
	private files = new Map<string, string>();
	private count = 0;
	private request = 11;
	private thread: string;
	private turn: string;
	private tail: Entry[];

	constructor(private seed: string, prompt: string) {
		const lines: Entry[] = readFileSync(recorded, 'utf8').split('\n').filter(Boolean).map((line) => JSON.parse(line));
		const opening = lines.findIndex((entry) => entry.stdout?.method === 'item/completed' && entry.stdout.params.item.type === 'userMessage');
		const closing = lines.findIndex((entry) => entry.stdout?.method === 'thread/status/changed' && entry.stdout.params.status.type === 'idle');
		this.entries = lines.slice(0, opening + 1);
		this.tail = lines.slice(closing);
		for (const entry of this.entries) {
			const message = entry.stdin ?? entry.stdout;
			if (message.method === 'turn/start') message.params.input[0].text = prompt;
			if (message.params?.item?.type === 'userMessage') message.params.item.content[0].text = prompt;
		}
		const started = this.entries.find((entry) => entry.stdout?.method === 'turn/started')!.stdout.params;
		this.thread = started.threadId;
		this.turn = started.turn.id;
	}

	private out(method: string, params: Entry) {
		this.entries.push({ stdout: { method, params: { threadId: this.thread, turnId: this.turn, ...params }, emittedAtMs: Date.now() } });
	}

	sleep(seconds: number) {
		this.entries.push({ sleep: seconds });
		return this;
	}

	say(text: string, phase = 'commentary') {
		const id = `msg_promo_${++this.count}`;
		const item = { type: 'agentMessage', id, text: '', phase, memoryCitation: null, delivery: null, questions: null };
		this.out('item/started', { item });
		for (const delta of text.match(/\S+\s*/g)!.reduce<string[]>((parts, word, index) => (index % 3 ? (parts[parts.length - 1] += word) : parts.push(word), parts), [])) {
			this.out('item/agentMessage/delta', { itemId: id, delta });
			this.sleep(0.05);
		}
		this.out('item/completed', { item: { ...item, text } });
		return this.sleep(0.3);
	}

	plan(steps: [string, 'pending' | 'inProgress' | 'completed'][]) {
		this.out('turn/plan/updated', { explanation: null, plan: steps.map(([step, status]) => ({ step, status })) });
		return this.sleep(0.2);
	}

	private command(command: string, actions: Entry[], output: string) {
		const id = `exec-promo-${++this.count}`;
		const item = { type: 'commandExecution', id, pluginId: null, scriptPath: null, command: `/usr/bin/zsh -lc '${command}'`, cwd, processId: '4242', source: 'unifiedExecStartup', status: 'inProgress', commandActions: actions, aggregatedOutput: null, exitCode: null, durationMs: null };
		return { id, item, done: { ...item, status: 'completed', aggregatedOutput: output, exitCode: 0, durationMs: 40 } };
	}

	private file(path: string) {
		if (!this.files.has(path)) this.files.set(path, readFileSync(join(this.seed, path), 'utf8'));
		return this.files.get(path)!;
	}

	read(path: string) {
		const command = `sed -n '1,80p' ${path}`;
		const { item, done } = this.command(command, [{ type: 'read', command, name: path.split('/').at(-1), path }], this.file(path));
		this.out('item/started', { item });
		this.sleep(0.3);
		this.out('item/completed', { item: done });
		return this.sleep(0.2);
	}

	edit(path: string, before: string, after: string) {
		const original = this.file(path);
		if (original.split(before).length !== 2) throw new Error(`${path} must contain one ${JSON.stringify(before)}`);
		this.files.set(path, original.replace(before, after));
		const [hunk] = patch(original, before, after);
		const diff = `@@ -${hunk.oldStart},${hunk.oldLines} +${hunk.newStart},${hunk.newLines} @@\n${hunk.lines.join('\n')}\n`;
		const item = { type: 'fileChange', id: `exec-promo-${++this.count}`, changes: [{ path: `${cwd}/${path}`, kind: { type: 'update', move_path: null }, diff }], status: 'inProgress' };
		this.out('item/started', { item });
		this.sleep(0.4);
		this.out('item/completed', { item: { ...item, status: 'completed' } });
		return this.sleep(0.3);
	}

	approve(command: string, reason: string, output: string) {
		const { id, item, done } = this.command(command, [{ type: 'unknown', command }], output);
		this.out('thread/status/changed', { status: { type: 'active', activeFlags: ['waitingOnApproval'] } });
		this.out('item/started', { item });
		const request = this.request++;
		this.entries.push({ stdout: { method: 'item/commandExecution/requestApproval', id: request, params: { kind: 'command', threadId: this.thread, turnId: this.turn, itemId: id, startedAtMs: Date.now(), environmentId: 'local', reason, command: item.command, cwd, commandActions: item.commandActions } } });
		this.entries.push({ stdin: { id: request, result: { decision: 'accept' } } });
		this.out('serverRequest/resolved', { requestId: request });
		this.out('thread/status/changed', { status: { type: 'active', activeFlags: [] } });
		this.sleep(1.2);
		this.out('item/completed', { item: done });
		return this.sleep(0.4);
	}

	end(summary: string) {
		this.say(summary, 'final_answer');
		for (const entry of this.tail) {
			if (entry.stdout?.method === 'turn/completed') entry.stdout.params.turn.items = [];
		}
		this.entries.push(...this.tail);
		return this;
	}

	write(path: string) {
		writeFileSync(path, this.entries.map((entry) => JSON.stringify(entry)).join('\n') + '\n');
	}
}
