import { templates } from '#lib/config.svelte.ts';
import type { Capabilities, Choice, Setting } from '#lib/device.svelte.ts';

const settings: Setting[] = ['effort', 'model', 'speed', 'ultracode', 'mode'];
export function builtins(capabilities: Capabilities) {
	return [...settings, ...(capabilities.compacts ? ['compact'] : []), ...(capabilities.rewinds ? ['rewind'] : [])];
}

export function describe(command: 'compact' | 'rewind', capabilities: Capabilities) {
	if (command === 'compact') return 'free up context';
	return capabilities.rewindsFiles ? 'conversation and tracked files' : 'conversation only; files unchanged';
}

export function offered(setting: Setting, choices: Record<Setting, Choice[]>) {
	return (setting !== 'speed' && setting !== 'ultracode') || choices[setting].length > 0;
}

/** Finds the slash command that leads `text`, if Stampede or the Agent knows it. */
export function recognize(text: string, capabilities: Capabilities, choices: Record<Setting, Choice[]>) {
	const [, name, value] = text.match(/^\/(\S+)(?: +(\S+))?/) ?? [];
	if (!name) return;
	const token = `/${name}`;
	if ((name === 'compact' || name === 'rewind') && builtins(capabilities).includes(name)) return { token, builtin: true, detail: describe(name, capabilities) };
	const setting = settings.find((setting) => setting === name && offered(setting, choices));
	if (setting) return { token, builtin: true, detail: choices[setting].find((choice) => choice.value === value || choice.label === value)?.label ?? '' };
	const template = templates().find((template) => template.name === name);
	if (template) return { token, builtin: false, detail: template.description };
	const command = capabilities.commands.find((command) => command.name === name);
	if (command) return { token, builtin: false, detail: command.description || command.hint };
}
