import { AbsoluteFill, interpolate, useCurrentFrame } from 'remotion';
import { cuts, fps, start, type Section } from './edit.ts';
import { colors, display, text } from './parts.tsx';
import { Extras } from './tour/Extras.tsx';
import { Grill } from './tour/Grill.tsx';
import { Worktrees } from './tour/Worktrees.tsx';
import { Delegate } from './tour/Delegate.tsx';
import { Terminals } from './tour/Terminals.tsx';
import { Editing } from './tour/Editor.tsx';
import { Committing } from './tour/Git.tsx';
import { Media } from './tour/Media.tsx';
import { Canvas } from './tour/Canvas.tsx';
import { Rigs } from './tour/Rigs.tsx';
import { Light } from './tour/Light.tsx';
import { Chatting, Edit, Fleet, Handset, Reviewing, Running, Splitting, Themes, Title } from './Promo.tsx';

const chapters: { name: string; sections: Section[] }[] = [
	{ name: 'Agents', sections: ['rigs', 'chat', 'phone', 'split', 'delegate'] },
	{ name: 'See the work', sections: ['media', 'canvas'] },
	{ name: 'Plan', sections: ['grill', 'epic', 'worktrees'] },
	{ name: 'Code', sections: ['editor', 'review', 'git'] },
	{ name: 'Everywhere', sections: ['terminals', 'fleet', 'light', 'extras', 'themes'] }
];

const shown: [Section, Section][] = [['rigs', 'phone'], ['split', 'outro']];

const Contents = () => {
	const now = useCurrentFrame() / fps;
	const clamp = { extrapolateLeft: 'clamp', extrapolateRight: 'clamp' } as const;
	const opacity = Math.max(...shown.map(([from, to]) => interpolate(now, [start('Tour', from), start('Tour', from) + 0.5, start('Tour', to) - 0.5, start('Tour', to)], [0, 1, 1, 0], clamp)));
	return (
		<AbsoluteFill style={{ opacity, justifyContent: 'center', paddingLeft: 40, gap: 26 }}>
			{chapters.map((chapter, index) => {
				const begins = start('Tour', chapter.sections[0]);
				const ends = start('Tour', cuts.Tour[cuts.Tour.indexOf(chapter.sections.at(-1)!) + 1]);
				const on = interpolate(now, [begins, begins + 0.4, ends, ends + 0.4], [0, 1, 1, 0], clamp);
				return (
					<div key={chapter.name} style={{ display: 'flex', alignItems: 'center', gap: 12, opacity: 0.4 + on * 0.6 }}>
						<div style={{ width: 4, height: 34, borderRadius: 2, background: colors.ember, transform: `scaleY(${on})` }} />
						<div>
							<div style={{ fontFamily: text, fontWeight: 600, fontSize: 15, color: colors.ember, letterSpacing: '0.08em' }}>{String(index + 1).padStart(2, '0')}</div>
							<div style={{ fontFamily: display, fontWeight: 700, fontSize: 25, color: on > 0.5 ? colors.ink : colors.muted, letterSpacing: '-0.01em' }}>{chapter.name}</div>
						</div>
					</div>
				);
			})}
		</AbsoluteFill>
	);
};

export const Tour = () => (
	<Edit cut="Tour" parts={{
		intro: <Title tagline="A self-hosted IDE for directing AI coding agents." />,
		rigs: <Rigs />,
		chat: <Chatting />,
		phone: <Handset />,
		split: <Splitting />,
		delegate: <Delegate />,
		media: <Media />,
		canvas: <Canvas />,
		grill: <Grill />,
		epic: <Running />,
		worktrees: <Worktrees />,
		editor: <Editing />,
		review: <Reviewing />,
		git: <Committing />,
		terminals: <Terminals />,
		fleet: <Fleet />,
		light: <Light />,
		extras: <Extras />,
		themes: <Themes />,
		outro: <Title tagline="Your machine. Your agents. Any browser." footer="github.com/d-flood/stampede" />
	}}>
		<Contents />
	</Edit>
);
