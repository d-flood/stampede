import type { CSSProperties, ReactNode } from 'react';
import { AbsoluteFill, Img, interpolate, spring, useCurrentFrame, useVideoConfig } from 'remotion';
import { loadFont as loadDisplay } from '@remotion/google-fonts/BricolageGrotesque';
import { loadFont as loadText } from '@remotion/google-fonts/Inter';
import logo from '../../web/static/icons/icon-512.png';

export const display = loadDisplay('normal', { weights: ['500', '700', '800'], subsets: ['latin'] }).fontFamily;
export const text = loadText('normal', { weights: ['300', '400', '500', '600'], subsets: ['latin'] }).fontFamily;

export const colors = {
	ink: '#f7efe6',
	muted: '#b9ab9c',
	ember: '#ec6f2d',
	ground: '#120f0d'
};

export const Backdrop = () => {
	const frame = useCurrentFrame();
	const drift = Math.sin(frame / 90) * 60;
	return (
		<AbsoluteFill style={{ background: `radial-gradient(1100px 700px at ${30 + drift / 20}% 18%, rgba(236, 111, 45, 0.22), transparent 70%), radial-gradient(900px 600px at 85% 100%, rgba(120, 72, 40, 0.28), transparent 70%), ${colors.ground}` }} />
	);
};

export function useFade(duration: number, edge = 8) {
	const frame = useCurrentFrame();
	return interpolate(frame, [0, edge, duration - edge, duration], [0, 1, 1, 0], { extrapolateLeft: 'clamp', extrapolateRight: 'clamp' });
}

export const Caption = ({ from, to, children, style }: { from: number; to: number; children: ReactNode; style?: CSSProperties }) => {
	const frame = useCurrentFrame();
	const { fps } = useVideoConfig();
	if (frame < from || frame >= to) return null;
	const enter = spring({ frame: frame - from, fps, config: { damping: 18, mass: 0.6 } });
	const leave = interpolate(frame, [to - 8, to], [1, 0], { extrapolateLeft: 'clamp' });
	return (
		<div style={{ position: 'absolute', opacity: enter * leave, transform: `translateY(${(1 - enter) * 24}px)`, fontFamily: display, color: colors.ink, letterSpacing: '-0.02em', ...style }}>
			{children}
		</div>
	);
};

export const Logo = ({ size, style }: { size: number; style?: CSSProperties }) => <Img src={logo} style={{ width: size, height: size, ...style }} />;

export const Browser = ({ width, height, url, children }: { width: number; height: number; url: string; children: ReactNode }) => (
	<div style={{ width, borderRadius: 16, overflow: 'hidden', background: '#1b1c1f', boxShadow: '0 40px 120px rgba(0,0,0,0.55), 0 0 0 1px rgba(255,255,255,0.08)' }}>
		<div style={{ height: 40, display: 'flex', alignItems: 'center', gap: 8, padding: '0 16px', background: '#26272b' }}>
			{['#ff5f57', '#febc2e', '#28c840'].map((color) => <div key={color} style={{ width: 12, height: 12, borderRadius: 6, background: color, opacity: 0.85 }} />)}
			<div style={{ marginLeft: 18, flex: 1, maxWidth: 520, height: 26, borderRadius: 13, background: '#1b1c1f', color: colors.muted, fontFamily: text, fontSize: 14, display: 'flex', alignItems: 'center', padding: '0 14px' }}>{url}</div>
		</div>
		<div style={{ position: 'relative', width, height, overflow: 'hidden' }}>{children}</div>
	</div>
);

const StatusBar = ({ color }: { color: string }) => (
	<div style={{ height: 30, display: 'flex', alignItems: 'center', justifyContent: 'space-between', padding: '0 22px', color, fontFamily: text, fontWeight: 500, fontSize: 14 }}>
		<span>9:41</span>
		<svg width={58} height={14} viewBox="0 0 58 14" fill={color}>
			<path d="M0 12h3v2H0zM5 9h3v5H5zM10 6h3v8h-3zM15 3h3v11h-3z" />
			<path d="M29 13.5l-7-7a10 10 0 0 1 14 0z" />
			<rect x={40} y={2} width={16} height={10} rx={2.5} fill="none" stroke={color} strokeWidth={1.4} />
			<rect x={42} y={4} width={10} height={6} rx={1} />
		</svg>
	</div>
);

export const Phone = ({ width, height, children, bar = '#16181b', barColor = colors.ink }: { width: number; height: number; children: ReactNode; bar?: string; barColor?: string }) => (
	<div style={{ padding: 12, borderRadius: 56, background: 'linear-gradient(160deg, #3a3633, #1c1a18)', boxShadow: '0 50px 120px rgba(0,0,0,0.6), inset 0 0 0 1.5px rgba(255,255,255,0.12)' }}>
		<div style={{ position: 'relative', width, height: height + 30, borderRadius: 44, overflow: 'hidden', background: bar }}>
			<StatusBar color={barColor} />
			<div style={{ position: 'relative', width, height, overflow: 'hidden' }}>{children}</div>
			<div style={{ position: 'absolute', top: 9, left: width / 2 - 7, width: 14, height: 14, borderRadius: 7, background: '#050505' }} />
		</div>
	</div>
);

export const LockScreen = ({ width, height, notify, open }: { width: number; height: number; notify: number; open: number }) => {
	const frame = useCurrentFrame();
	const { fps } = useVideoConfig();
	const arrive = spring({ frame: frame - notify * fps, fps, config: { damping: 16 } });
	const press = interpolate(frame, [open * fps - 4, open * fps, open * fps + 4], [1, 0.96, 1], { extrapolateLeft: 'clamp', extrapolateRight: 'clamp' });
	return (
		<AbsoluteFill style={{ width, height, background: 'radial-gradient(420px 520px at 20% 15%, #b0532a, transparent 70%), radial-gradient(420px 520px at 90% 80%, #1f4f5c, transparent 70%), #17120f', fontFamily: text, color: '#fff' }}>
			<div style={{ textAlign: 'center', marginTop: 120, fontSize: 112, fontWeight: 300, lineHeight: 0.9, letterSpacing: '-0.03em' }}>9<br />41</div>
			<div style={{ textAlign: 'center', marginTop: 18, fontSize: 18, opacity: 0.85 }}>Tue, Apr 14</div>
			<div style={{ position: 'absolute', left: 12, right: 12, top: 40, opacity: arrive, transform: `translateY(${(1 - arrive) * -120}px) scale(${press})`, background: 'rgba(32, 30, 29, 0.94)', borderRadius: 24, padding: '14px 16px', boxShadow: '0 10px 30px rgba(0,0,0,0.4)' }}>
				<div style={{ display: 'flex', alignItems: 'center', gap: 8, fontSize: 15, color: '#cfc6be' }}>
					<Logo size={22} style={{ borderRadius: 5, background: '#fff' }} />
					<span>Studio</span>
					<span style={{ opacity: 0.6 }}>· now</span>
				</div>
				<div style={{ marginTop: 6, fontSize: 21, fontWeight: 600 }}>Claude — waiting</div>
			</div>
		</AbsoluteFill>
	);
};
