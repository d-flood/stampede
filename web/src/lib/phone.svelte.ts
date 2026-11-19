import { goto } from '$app/navigation';
import { page } from '$app/state';

/** What slides over a phone's current Tab: the Project rail from the left or the Sidebar from the right. */
export type Panel = 'projects' | 'sidebar';

let dismissed = $state.raw<App.PageState>();

export function panel(): Panel | undefined {
	// Closing waits on a history traversal, so the panel hides right away instead of when it lands.
	return page.state === dismissed ? undefined : page.state.panel;
}

/** Opens a panel as its own history entry, so the system back gesture closes it. */
export function openPanel(target: Panel) {
	if (panel() === target) return;
	return goto(page.url.href, { shallow: true, replace: !!panel(), persistState: true, state: { panel: target } });
}

export function closePanel() {
	if (!panel()) return;
	dismissed = page.state;
	// Iframes add their own entries to joint session history, so history.back could step a canvas back instead of this page.
	const entries = 'navigation' in window ? navigation.entries() : undefined;
	const previous = entries?.[(navigation.currentEntry?.index ?? 0) - 1];
	if (previous) navigation.traverseTo(previous.key);
	else if (entries) goto(page.url.href, { shallow: true, replace: true, persistState: true, state: {} });
	else history.back();
}
