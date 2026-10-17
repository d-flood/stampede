import type { Extension } from '@codemirror/state';

export async function keybindings(name: 'standard' | 'vim'): Promise<Extension> {
	return name === 'vim' ? (await import('@replit/codemirror-vim')).vim() : [];
}
