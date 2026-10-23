export type SidebarSide = 'left' | 'right';

const key = 'stampede.sidebar-side';
export const sidebarPosition = $state<{ side: SidebarSide }>({ side: 'left' });

export function loadSidebarSide() {
	try {
		sidebarPosition.side = localStorage.getItem(key) === 'right' ? 'right' : 'left';
	} catch {
		sidebarPosition.side = 'left';
	}
}

export function selectSidebarSide(side: SidebarSide) {
	sidebarPosition.side = side;
	try {
		localStorage.setItem(key, side);
	} catch {}
}
