import { readFileSync, writeFileSync } from 'node:fs';
import { randomUUID } from 'node:crypto';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';

const recorded = fileURLToPath(new URL('../../crates/stampede/tests/fixtures/chats/claude-code/', import.meta.url));
const cwd = '/tmp/stampede-chat';
const commands = ['clear', 'compact', 'context', 'effort', 'init', 'model', 'review', 'security-review', 'usage'];

type Entry = Record<string, unknown>;

const absolute = (path: string) => (path.startsWith('/') ? path : `${cwd}/${path}`);

function template(name: string, find: (message: any) => boolean) {
	for (const line of readFileSync(join(recorded, name), 'utf8').split('\n').filter(Boolean)) {
		const entry = JSON.parse(line);
		if (entry.stdout && find(entry.stdout)) return entry.stdout;
	}
	throw new Error(`No template in ${name}`);
}

function chunks(text: string) {
	return text.match(/\S+\s*|\s+/g)!.reduce<string[]>((parts, word, index) => {
		if (index % 3 === 0) parts.push(word);
		else parts[parts.length - 1] += word;
		return parts;
	}, []);
}

export function patch(original: string, before: string, after: string) {
	const at = original.indexOf(before);
	const prefix = original.slice(0, at).split('\n').length - 1;
	let removed = before.split('\n');
	let added = after.split('\n');
	let start = prefix;
	while (removed.length && added.length && removed[0] === added[0]) {
		removed = removed.slice(1);
		added = added.slice(1);
		start++;
	}
	while (removed.length && added.length && removed.at(-1) === added.at(-1)) {
		removed = removed.slice(0, -1);
		added = added.slice(0, -1);
	}
	const lines = original.split('\n');
	const head = lines.slice(Math.max(0, start - 3), start);
	const tail = lines.slice(start + removed.length, start + removed.length + 3);
	return [{
		oldStart: start - head.length + 1,
		oldLines: head.length + removed.length + tail.length,
		newStart: start - head.length + 1,
		newLines: head.length + added.length + tail.length,
		lines: [...head.map((line) => ` ${line}`), ...removed.map((line) => `-${line}`), ...added.map((line) => `+${line}`), ...tail.map((line) => ` ${line}`)]
	}];
}

export type Question = { header: string; question: string; options: { label: string; description?: string; preview?: string }[]; multiSelect: boolean };

export class Chat {
	entries: Entry[] = [];
	private files = new Map<string, string>();
	private count = 0;
	private session = randomUUID();
	private parent: string | null = null;

	constructor(private seed: string, private pace = 1) {
		const initialize = template('simple-turn.recorded.jsonl', (message) => message.type === 'control_response');
		initialize.response.request_id = 'init';
		initialize.response.response.commands = initialize.response.response.commands.filter((command: { name: string }) => commands.includes(command.name));
		initialize.response.response.account = { subscriptionType: 'Claude Max', apiProvider: 'firstParty' };
		this.entries.push({ stdin: { type: 'control_request', request_id: 'init', request: { subtype: 'initialize' } } }, { stdout: initialize });
	}

	private out(message: Entry) {
		this.entries.push({ stdout: { ...message, session_id: this.session, uuid: randomUUID() } });
	}

	private event(event: Entry, message: string) {
		this.out({ type: 'stream_event', event, parent_tool_use_id: null, api_message_id: message });
	}

	sleep(seconds: number) {
		this.entries.push({ sleep: seconds * this.pace });
		return this;
	}

	prompt() {
		this.entries.push({ stdin: { type: 'user' } });
		this.out({ type: 'system', subtype: 'session_state_changed', state: 'running', sdk_host_only: true });
		const init = template('simple-turn.recorded.jsonl', (message) => message.subtype === 'init');
		this.out({ ...init, cwd, original_cwd: cwd });
		this.out({ type: 'rate_limit_event', rate_limit_info: { status: 'allowed', rateLimitType: 'five_hour', unifiedWindows: { five_hour: { utilization: 0.34, resetsAt: Math.floor(Date.now() / 1000) + 9_000 }, seven_day: { utilization: 0.52, resetsAt: Math.floor(Date.now() / 1000) + 300_000 } } } });
		return this.sleep(0.8);
	}

	private message(block: Entry, deltas: Entry[], stop: string, delay = 0.05) {
		const id = `msg_promo_${++this.count}`;
		const assistant = { type: 'assistant', message: { model: 'claude-opus-5-5', id, type: 'message', role: 'assistant', content: [block], stop_reason: null }, parent_tool_use_id: this.parent };
		if (this.parent) {
			this.out(assistant);
			return;
		}
		this.event({ type: 'message_start', message: { model: 'claude-opus-5-5', id, type: 'message', role: 'assistant', content: [], usage: { input_tokens: 2, cache_read_input_tokens: 24_000 + this.count * 1_500, output_tokens: 1 } } }, id);
		this.event({ type: 'content_block_start', index: 0, content_block: block.type === 'text' ? { type: 'text', text: '' } : { ...block, input: {} } }, id);
		for (const delta of deltas) {
			this.event({ type: 'content_block_delta', index: 0, delta }, id);
			this.sleep(delay);
		}
		this.out(assistant);
		this.event({ type: 'content_block_stop', index: 0 }, id);
		this.event({ type: 'message_delta', delta: { stop_reason: stop }, usage: { output_tokens: 120 } }, id);
		this.event({ type: 'message_stop' }, id);
	}

	say(text: string) {
		this.message({ type: 'text', text }, chunks(text).map((part) => ({ type: 'text_delta', text: part })), 'end_turn');
		return this.sleep(0.3);
	}

	private use(name: string, input: Entry, id = `toolu_promo_${++this.count}`) {
		const json = JSON.stringify(input);
		const deltas = json.match(/.{1,64}/gs)!.map((part) => ({ type: 'input_json_delta', partial_json: part }));
		this.message({ type: 'tool_use', id, name, input }, deltas, 'tool_use', 0.03);
		return id;
	}

	private result(id: string, content: unknown, result: unknown, error = false) {
		this.out({ type: 'user', message: { role: 'user', content: [{ tool_use_id: id, type: 'tool_result', content, is_error: error }] }, parent_tool_use_id: this.parent, timestamp: new Date().toISOString(), tool_use_result: result });
	}

	private file(path: string) {
		if (!this.files.has(path)) this.files.set(path, readFileSync(join(this.seed, path), 'utf8'));
		return this.files.get(path)!;
	}

	read(path: string) {
		const id = this.use('Read', { file_path: absolute(path) });
		const text = this.file(path);
		this.sleep(0.25);
		this.result(id, text.split('\n').map((line, index) => `${index + 1}\t${line}`).join('\n'), { type: 'text', file: { filePath: absolute(path), content: text, numLines: text.split('\n').length, startLine: 1, totalLines: text.split('\n').length } });
		return this.sleep(0.2);
	}

	grep(pattern: string, matches: string[]) {
		const id = this.use('Grep', { pattern, output_mode: 'files_with_matches' });
		this.sleep(0.25);
		this.result(id, `Found ${matches.length} files\n${matches.join('\n')}`, { mode: 'files_with_matches', filenames: matches, numFiles: matches.length });
		return this.sleep(0.2);
	}

	agent(description: string, type: string, prompt: string, work: (chat: Chat) => void, report: string) {
		const id = this.use('Agent', { description, subagent_type: type, prompt, model: 'claude-haiku-4-5' });
		this.sleep(0.3);
		this.parent = id;
		work(this);
		this.parent = null;
		this.result(id, [{ type: 'text', text: report }], { status: 'completed', agentId: id, content: [{ type: 'text', text: report }], totalDurationMs: 21_000, totalToolUseCount: 4, totalTokens: 31_000, resolvedModel: 'claude-haiku-4-5' });
		return this.sleep(0.3);
	}

	todos(items: [string, 'pending' | 'in_progress' | 'completed'][]) {
		const todos = items.map(([content, status]) => ({ content, status, activeForm: content }));
		const id = this.use('TodoWrite', { todos });
		this.result(id, 'Todos have been modified successfully. Ensure that you continue to use the todo list to track your progress. Please proceed with the current tasks if applicable', { oldTodos: [], newTodos: todos });
		return this.sleep(0.2);
	}

	html(title: string, html: string) {
		const id = this.use('mcp__stampede_chat__render_html', { title, html });
		this.result(id, [{ type: 'text', text: 'Rendered HTML in the chat.' }], null);
		return this.sleep(0.4);
	}

	ask(...rounds: [Question, string][]) {
		const questions = rounds.map(([question]) => question);
		const id = this.use('AskUserQuestion', { questions });
		this.out({ type: 'system', subtype: 'session_state_changed', state: 'requires_action', sdk_host_only: true });
		this.entries.push({ stdout: { type: 'control_request', request_id: id, request: { subtype: 'can_use_tool', tool_name: 'AskUserQuestion', input: { questions }, tool_use_id: id } } });
		this.entries.push({ stdin: { type: 'control_response', response: { request_id: id } } });
		this.out({ type: 'system', subtype: 'session_state_changed', state: 'running', sdk_host_only: true });
		this.result(id, `User has answered your questions: ${rounds.map(([question, answer]) => `"${question.question}"="${answer}"`).join(', ')}.`, { questions, answers: Object.fromEntries(rounds.map(([question, answer]) => [question.question, answer])) });
		return this.sleep(0.4);
	}

	media(path: string, title: string, asset: string) {
		const id = this.use('mcp__stampede_chat__render_media', { path, title });
		this.result(id, [{ type: 'text', text: JSON.stringify({ title, asset }) }], null);
		return this.sleep(0.4);
	}

	delegate(rig: string, task: string) {
		this.use('mcp__stampede_chat__delegate_to_rig', { rig, task }, 'rig-review');
		this.entries.push({ relay: { rig, task } });
		return this.sleep(0.4);
	}

	stage(command: string, gate: string) {
		const id = this.use('mcp__stampede_chat__stage_command', { command });
		this.entries.push({ gate });
		this.result(id, [{ type: 'text', text: JSON.stringify({ pending: true }) }], null);
		return this.sleep(0.4);
	}

	edit(path: string, before: string, after: string) {
		const original = this.file(path);
		if (original.split(before).length !== 2) throw new Error(`${path} must contain one ${JSON.stringify(before)}`);
		const updated = original.replace(before, after);
		const id = this.use('Edit', { replace_all: false, file_path: absolute(path), old_string: before, new_string: after });
		this.files.set(path, updated);
		this.entries.push({ file: { path, text: updated } });
		this.sleep(0.2);
		this.result(id, `The file ${absolute(path)} has been updated successfully.`, { filePath: absolute(path), oldString: before, newString: after, originalFile: original, structuredPatch: patch(original, before, after), userModified: false, replaceAll: false });
		return this.sleep(0.3);
	}

	create(path: string, text: string) {
		const id = this.use('Write', { file_path: absolute(path), content: text });
		this.files.set(path, text);
		this.entries.push({ file: { path, text } });
		this.sleep(0.2);
		this.result(id, `File created successfully at: ${absolute(path)}`, { type: 'create', filePath: absolute(path), content: text, structuredPatch: [], originalFile: null });
		return this.sleep(0.3);
	}

	bash(command: string, description: string, output: string) {
		const id = this.use('Bash', { command, description });
		this.sleep(0.8);
		this.result(id, output, { stdout: output, stderr: '', interrupted: false, isImage: false, noOutputExpected: false });
		return this.sleep(0.3);
	}

	approve(command: string, description: string, rule: string, output: string) {
		const id = this.use('Bash', { command, description });
		this.out({ type: 'system', subtype: 'session_state_changed', state: 'requires_action', sdk_host_only: true });
		this.entries.push({ stdout: { type: 'control_request', request_id: id, request: { subtype: 'can_use_tool', tool_name: 'Bash', display_name: 'Bash', input: { command, description }, description, permission_suggestions: [{ type: 'addRules', rules: [{ toolName: 'Bash', ruleContent: rule }], behavior: 'allow', destination: 'localSettings' }], tool_use_id: id } } });
		this.entries.push({ stdin: { type: 'control_response', response: { request_id: id } } });
		this.out({ type: 'system', subtype: 'session_state_changed', state: 'running', sdk_host_only: true });
		this.sleep(1.2);
		this.result(id, output, { stdout: output, stderr: '', interrupted: false, isImage: false, noOutputExpected: false });
		return this.sleep(0.4);
	}

	epic(action: Entry) {
		this.entries.push({ epic: action });
		return this;
	}

	run(...command: string[]) {
		this.entries.push({ run: command });
		return this;
	}

	end(summary: string) {
		const result = template('simple-turn.recorded.jsonl', (message) => message.type === 'result');
		this.out({ ...result, result: summary, duration_ms: 48_000, duration_api_ms: 41_000, num_turns: 9 });
		this.out({ type: 'system', subtype: 'session_state_changed', state: 'idle', sdk_host_only: true });
		return this;
	}

	write(path: string) {
		writeFileSync(path, this.entries.map((entry) => JSON.stringify(entry)).join('\n') + '\n');
	}
}
