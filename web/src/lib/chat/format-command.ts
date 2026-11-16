export function formatCommand(command: string): string {
	if (/[\r\n]/.test(command)) return command;
	const breaks: { from: number; to: number; indent: string }[] = [];
	let quote = '';
	let word = '';
	let head = true;
	const compound = /^(?:if|then|else|elif|fi|for|while|until|do|done|case|esac|function|select|in|\[\[|\]\])$/;
	for (let at = 0; at < command.length; at++) {
		const char = command[at];
		if (quote === "'") {
			if (char === quote) quote = '';
			continue;
		}
		if (char === '\\') {
			if (at + 1 === command.length) return command;
			at++;
			continue;
		}
		if (char === '`' || (char === '$' && /[({'"]/.test(command[at + 1] ?? ''))) return command;
		if (quote) {
			if (char === quote) quote = '';
			continue;
		}
		if (char === "'" || char === '"') { quote = char; word += char; continue; }
		if (/[#(){}]/.test(char)) return command;
		if (/\s/.test(char) || /[|&;<>]/.test(char)) {
			if (head && compound.test(word)) return command;
			if (word) head = false;
			word = '';
		} else {
			word += char;
			continue;
		}
		if (/\s/.test(char)) continue;
		const operator = command.slice(at).match(/^[|&;<>]+/)![0];
		at += operator.length - 1;
		if (/^(?:>>?|>&|>\||<&?|<>|<<<|&>>?)$/.test(operator)) {
			if (head) return command;
			continue;
		}
		if (!['|', '|&', '&&', '||', ';', '&'].includes(operator)) return command;
		head = true;
		const from = at + 1;
		let to = from;
		while (command[to] === ' ' || command[to] === '\t') to++;
		if (to < command.length) breaks.push({ from, to, indent: operator === ';' || operator === '&' ? '' : '  ' });
	}
	if (quote || (head && compound.test(word))) return command;
	let formatted = command;
	for (const { from, to, indent } of breaks.reverse()) formatted = `${formatted.slice(0, from)}\n${indent}${formatted.slice(to)}`;
	return formatted;
}
