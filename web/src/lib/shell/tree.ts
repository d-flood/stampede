import { split, type Change } from '#lib/git.ts';

export type Folder = { name: string; path: string; folders: Folder[]; files: Change[] };

export type Row =
	| { kind: 'folder'; key: string; folder: Folder; depth: number; changes: Change[]; open: boolean }
	| { kind: 'file'; key: string; change: Change; depth: number };

/** Changes as folder and file rows in display order, keyed under `prefix`, descending only into folders `open` allows. */
export function treeRows(changes: Change[], prefix: string, open: (key: string, changes: Change[]) => boolean): Row[] {
	return flatten(tree(changes), prefix, 0, open);
}

/** Every change in the order its tree shows it, with every folder open. */
export function ordered(changes: Change[]): Change[] {
	return treeRows(changes, '', () => true).flatMap((row) => (row.kind === 'file' ? [row.change] : []));
}

function tree(changes: Change[]): Folder {
	const root: Folder = { name: '', path: '', folders: [], files: [] };
	for (const change of changes) {
		let folder = root;
		for (const name of change.path.split('/').slice(0, -1)) {
			const path = folder.path ? `${folder.path}/${name}` : name;
			let child = folder.folders.find((candidate) => candidate.path === path);
			if (!child) folder.folders.push((child = { name, path, folders: [], files: [] }));
			folder = child;
		}
		folder.files.push(change);
	}
	return root;
}

function compact(folder: Folder): Folder {
	while (!folder.files.length && folder.folders.length === 1) {
		const [only] = folder.folders;
		folder = { ...only, name: `${folder.name}/${only.name}` };
	}
	return folder;
}

function under(folder: Folder): Change[] {
	return [...folder.files, ...folder.folders.flatMap(under)];
}

function flatten(folder: Folder, prefix: string, depth: number, open: (key: string, changes: Change[]) => boolean): Row[] {
	const folders = folder.folders.map(compact).sort((a, b) => a.name.localeCompare(b.name));
	const files = folder.files.toSorted((a, b) => split(a.path).name.localeCompare(split(b.path).name));
	return [
		...folders.flatMap((child): Row[] => {
			const key = `${prefix}:${child.path}/`;
			const changes = under(child);
			const shown = open(key, changes);
			const row: Row = { kind: 'folder', key, folder: child, depth, changes, open: shown };
			return shown ? [row, ...flatten(child, prefix, depth + 1, open)] : [row];
		}),
		...files.map((change): Row => ({ kind: 'file', key: `${prefix}:${change.path}`, change, depth }))
	];
}
