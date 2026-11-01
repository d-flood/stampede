import { Composition } from 'remotion';
import { Promo } from './Promo.tsx';
import { fps, total } from './edit.ts';

export const Root = () => (
	<Composition id="Promo" component={Promo} durationInFrames={Math.round(total * fps)} fps={fps} width={1920} height={1080} />
);
