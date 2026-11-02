import { Composition } from 'remotion';
import { Promo } from './Promo.tsx';
import { Tour } from './Tour.tsx';
import { fps, total } from './edit.ts';

export const Root = () => (
	<>
		<Composition id="Promo" component={Promo} durationInFrames={Math.round(total('Promo') * fps)} fps={fps} width={1920} height={1080} />
		<Composition id="Tour" component={Tour} durationInFrames={Math.round(total('Tour') * fps)} fps={fps} width={1920} height={1080} />
	</>
);
