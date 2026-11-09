const { createRequire } = require('node:module');
const { join } = require('node:path');
const { fileURLToPath, pathToFileURL } = require('node:url');

const root = process.cwd();
const documents = new Map();
let eslint;

function send(message) {
	const body = JSON.stringify(message);
	process.stdout.write(`Content-Length: ${Buffer.byteLength(body)}\r\n\r\n${body}`);
}

async function load() {
	const resolved = createRequire(join(root, 'package.json')).resolve('eslint');
	const module = await import(pathToFileURL(resolved).href);
	const api = module.default ?? module;
	const ESLint = (await api.loadESLint?.({ useFlatConfig: true })) ?? api.ESLint;
	return new ESLint({ cwd: root });
}

async function diagnose(uri) {
	const text = documents.get(uri);
	if (text === undefined) return [];
	try {
		eslint ??= load();
		const [result] = await (await eslint).lintText(text, { filePath: fileURLToPath(uri), warnIgnored: false });
		return (result?.messages ?? []).map((message) => {
			const line = (message.line ?? 1) - 1;
			const character = (message.column ?? 1) - 1;
			const end = { line: (message.endLine ?? message.line ?? 1) - 1, character: (message.endColumn ?? message.column ?? 1) - 1 };
			return {
				range: { start: { line, character }, end },
				severity: message.severity === 2 ? 1 : 2,
				source: 'ESLint',
				code: message.ruleId ?? undefined,
				message: message.message
			};
		});
	} catch (error) {
		eslint = undefined;
		const start = { line: 0, character: 0 };
		return [{ range: { start, end: start }, severity: 1, source: 'ESLint', message: String(error?.message ?? error) }];
	}
}

function handle({ id, method, params }) {
	switch (method) {
		case 'initialize':
			return send({
				jsonrpc: '2.0',
				id,
				result: { capabilities: { textDocumentSync: 1, diagnosticProvider: { interFileDependencies: false, workspaceDiagnostics: false } } }
			});
		case 'textDocument/didOpen':
			return documents.set(params.textDocument.uri, params.textDocument.text);
		case 'textDocument/didChange':
			return documents.set(params.textDocument.uri, params.contentChanges.at(-1).text);
		case 'textDocument/didClose':
			return documents.delete(params.textDocument.uri);
		case 'workspace/didChangeWatchedFiles':
			if (params.changes.some(({ uri }) => /\/eslint\.config\.[cm]?[jt]s$/.test(uri))) eslint = undefined;
			return;
		case 'textDocument/diagnostic':
			return diagnose(params.textDocument.uri).then((items) => send({ jsonrpc: '2.0', id, result: { kind: 'full', items } }));
		case 'shutdown':
			return send({ jsonrpc: '2.0', id, result: null });
		case 'exit':
			return process.exit(0);
		default:
			if (id !== undefined && method) send({ jsonrpc: '2.0', id, error: { code: -32601, message: `${method} is not supported` } });
	}
}

let buffer = Buffer.alloc(0);
process.stdin.on('data', (chunk) => {
	buffer = Buffer.concat([buffer, chunk]);
	for (;;) {
		const end = buffer.indexOf('\r\n\r\n');
		if (end < 0) return;
		const length = Number(/content-length:\s*(\d+)/i.exec(buffer.subarray(0, end).toString())?.[1]);
		if (buffer.length < end + 4 + length) return;
		const body = buffer.subarray(end + 4, end + 4 + length).toString();
		buffer = buffer.subarray(end + 4 + length);
		handle(JSON.parse(body));
	}
});
process.stdin.on('end', () => process.exit(0));
