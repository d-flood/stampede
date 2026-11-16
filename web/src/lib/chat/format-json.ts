export function formatJson(code: string): string | undefined {
	try { JSON.parse(code); } catch { return undefined; }
	// Preserve numeric precision and duplicate keys.
	const tokens = code.match(/"(?:[^"\\]|\\.)*"|[^\s{}\[\],:]+|[{}\[\],:]/g) ?? [];
	let formatted = '';
	let depth = 0;
	const newline = () => `\n${'  '.repeat(depth)}`;
	for (let at = 0; at < tokens.length; at++) {
		const token = tokens[at];
		if (token === '{' || token === '[') {
			formatted += token;
			depth++;
			if (tokens[at + 1] !== (token === '{' ? '}' : ']')) formatted += newline();
		} else if (token === '}' || token === ']') {
			depth--;
			if (tokens[at - 1] !== (token === '}' ? '{' : '[')) formatted += newline();
			formatted += token;
		} else if (token === ',') formatted += `,${newline()}`;
		else if (token === ':') formatted += ': ';
		else formatted += token;
	}
	return formatted;
}
