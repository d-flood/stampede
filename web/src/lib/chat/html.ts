import type { Item } from '#lib/device.svelte.ts';
import { createContext } from 'svelte';

export type HtmlItem = Extract<Item, { kind: 'html' }>;

export const [getHtmlPreview, setHtmlPreview] = createContext<(item: HtmlItem) => void>();
