import type { Chat, ClientMessage, ServerMessage, Terminal } from '#lib/device.svelte.ts';
import { projectId, remoteId } from '#lib/host.ts';

/** Namespaces a Host's Project ids. Undefined when only the Home Host may say it. */
export function incoming(message: ServerMessage, host?: string): ServerMessage | undefined {
	if (message.type === 'projects.changed') {
		return host ? undefined : { ...message, projects: message.projects.map((project) => ({ ...project, id: projectId(project.id, project.host) })) };
	}
	if (message.type === 'attention.changed' && !host) {
		return { ...message, items: message.items.map((item) => ({ ...item, projectId: projectId(item.projectId, item.host) })) };
	}
	if (!host) return message;
	const named = (id: string) => projectId(id, host);
	const chat = (chat: Chat) => ({ ...chat, projectId: named(chat.projectId) });
	const terminal = (terminal: Terminal) => (terminal.owner === 'drawer' ? terminal : { ...terminal, owner: { project: named(terminal.owner.project) } });
	switch (message.type) {
		case 'config.changed':
		case 'usage.changed':
		case 'attention.changed':
		case 'project.opened':
		case 'project.error':
		case 'picker.listed':
			return;
		case 'hello':
			return { ...message, terminals: message.terminals.map(terminal), chats: message.chats.map(chat) };
		case 'terminal.created':
			return { ...message, terminal: terminal(message.terminal) };
		case 'terminals.changed':
			return message.projectId ? { ...message, projectId: named(message.projectId), terminals: message.terminals.map(terminal) } : undefined;
		case 'chat.created':
		case 'chat.moved':
			return { ...message, chat: chat(message.chat) };
		case 'chats.changed':
			return { ...message, projectId: named(message.projectId), chats: message.chats.map(chat) };
		case 'origin.changed':
			return { ...message, trunks: Object.fromEntries(Object.entries(message.trunks).map(([id, trunk]) => [named(id), trunk])) };
		case 'lsp':
		case 'lsp.state':
			return { ...message, channel: rename(message.channel, named) };
		default:
			return 'projectId' in message ? { ...message, projectId: named(message.projectId) } : message;
	}
}

export function outgoing(message: ClientMessage, host?: string): ClientMessage {
	if (!host) return message;
	if (message.type === 'lsp' || message.type === 'lsp.open' || message.type === 'lsp.close') return { ...message, channel: rename(message.channel, remoteId) };
	return 'projectId' in message ? { ...message, projectId: remoteId(message.projectId) } : message;
}

function rename(channel: string, name: (id: string) => string): string {
	return channel.replace(/(?<=^lsp:)[^:]+/, name);
}
