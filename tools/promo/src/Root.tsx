import { Composition } from 'remotion';
import { Promo } from './Promo';

export const Root = () => (
    <>
        <Composition
            id="PromoLandscape"
            component={Promo}
            durationInFrames={1680}
            fps={30}
            width={1920}
            height={1080}
            defaultProps={{ portrait: false, audio: true }}
        />
        <Composition
            id="PromoPhone"
            component={Promo}
            durationInFrames={1680}
            fps={30}
            width={1080}
            height={1920}
            defaultProps={{ portrait: true, audio: true }}
        />
    </>
);
