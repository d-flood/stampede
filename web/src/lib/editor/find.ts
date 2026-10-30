import FindPanel from '#lib/editor/FindPanel.svelte';
import { search, setSearchQuery } from '@codemirror/search';
import { mount, unmount } from 'svelte';

export const find = search({
	createPanel(view) {
		const dom = document.createElement('div');
		const panel = mount(FindPanel, { target: dom, props: { view } });
		return {
			dom,
			mount: () => dom.querySelector<HTMLInputElement>('[main-field]')?.select(),
			update(update) {
				for (const transaction of update.transactions)
					for (const effect of transaction.effects) if (effect.is(setSearchQuery)) panel.sync(effect.value);
			},
			destroy: () => unmount(panel)
		};
	}
});
