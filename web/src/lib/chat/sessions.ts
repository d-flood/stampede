import type { Agent } from '#lib/config.svelte.ts';
import type { Component } from 'svelte';
import Claude from '~icons/simple-icons/claude';
import OpenAI from '~icons/simple-icons/openai';

export const logos: Record<Agent, Component> = { 'claude-code': Claude, codex: OpenAI };

const units: [Intl.RelativeTimeFormatUnit, number][] = [
	['year', 31_536_000],
	['month', 2_592_000],
	['week', 604_800],
	['day', 86_400],
	['hour', 3_600],
	['minute', 60]
];
const relative = new Intl.RelativeTimeFormat(undefined, { numeric: 'auto', style: 'short' });

export function ago(time: number): string {
	const seconds = (time - Date.now()) / 1000;
	const [unit, size] = units.find(([, size]) => Math.abs(seconds) >= size) ?? units[units.length - 1];
	return relative.format(Math.round(seconds / size), unit);
}
