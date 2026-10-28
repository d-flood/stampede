import type { Item } from '#lib/device.svelte.ts';
import { createContext } from 'svelte';

export type HtmlItem = Extract<Item, { kind: 'html' }>;

export const [getHtmlPreview, setHtmlPreview] = createContext<(item: HtmlItem) => void>();

export function sandboxed(html: string, script: string) {
	return `<!doctype html>
<html><head><meta charset="utf-8">
<meta http-equiv="Content-Security-Policy" content="default-src 'none'; script-src 'unsafe-inline'; style-src 'unsafe-inline'; img-src data:; font-src data:; connect-src 'none'; frame-src 'none'; form-action 'none'; base-uri 'none'">
<meta name="viewport" content="width=device-width, initial-scale=1">
<style>:root{color-scheme:light dark;font:14px/1.5 system-ui}*{box-sizing:border-box}body{margin:0;padding:16px;overflow-wrap:anywhere;display:flow-root}img,svg{max-width:100%}</style>
<script>${script}</script></head><body>${html}</body></html>`;
}
