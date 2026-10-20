import { useEffect, useState } from 'react';
import { Audio, Video } from '@remotion/media';
import { loadFont } from '@remotion/fonts';
import {
    AbsoluteFill,
    CanvasImage,
    Easing,
    Freeze,
    Sequence,
    cancelRender,
    continueRender,
    delayRender,
    interpolate,
    staticFile,
    useCurrentFrame,
    useVideoConfig,
} from 'remotion';

const C = { bg: '#0b0e11', text: '#e8ecf1', muted: '#aab4bf', gold: '#f0b90b', red: '#ff7784', line: '#29313c' };
const ease = { extrapolateLeft: 'clamp', extrapolateRight: 'clamp', easing: Easing.bezier(0.16, 1, 0.3, 1) } as const;
type Caption = { text: string; startMs: number; endMs: number };
type Props = { portrait: boolean; audio?: boolean };
type Crop = [number, number, number, number];

function Background() {
    const frame = useCurrentFrame();
    return (
        <AbsoluteFill style={{ background: C.bg, overflow: 'hidden' }}>
            <AbsoluteFill style={{ background: 'radial-gradient(ellipse at 90% 35%, #38301a66, transparent 62%)' }} />
            <AbsoluteFill
                style={{
                    opacity: 0.12,
                    backgroundImage:
                        'linear-gradient(#5b637344 1px, transparent 1px),linear-gradient(90deg,#5b637344 1px,transparent 1px)',
                    backgroundSize: '100px 100px',
                    translate: `${(frame / 8) % 100}px 0px`,
                }}
            />
            <div
                style={{
                    position: 'absolute',
                    width: 950,
                    height: 950,
                    border: '1px solid #f0b90b16',
                    borderRadius: '50%',
                    right: -300,
                    top: 140,
                    scale: interpolate(frame, [0, 1680], [1, 1.2], ease),
                }}
            />
        </AbsoluteFill>
    );
}

function Brand({ portrait }: { portrait: boolean }) {
    return (
        <div
            style={{
                position: 'absolute',
                top: portrait ? 102 : 64,
                left: 80,
                right: portrait ? 96 : 80,
                display: 'flex',
                alignItems: 'center',
                justifyContent: 'space-between',
            }}
        >
            <CanvasImage
                src={staticFile('brand/lockup-two-tone.svg')}
                style={{ width: portrait ? 310 : 282, height: 72, objectFit: 'contain' }}
            />
            <div style={{ fontSize: portrait ? 24 : 22, letterSpacing: 2, color: C.muted }}>PRODUCT DEMO</div>
        </div>
    );
}

function Footage({
    portrait,
    source,
    crop,
    phoneCrop,
    label,
    hero = false,
    contain = false,
}: {
    portrait: boolean;
    source: [number, number];
    crop: Crop;
    phoneCrop: Crop;
    label: string;
    hero?: boolean;
    contain?: boolean;
}) {
    const frame = useCurrentFrame();
    const { fps } = useVideoConfig();
    const [x, y, w, h] = portrait ? phoneCrop : crop;
    const box = portrait
        ? { left: 80, top: hero ? 660 : 490, width: 920, height: hero ? 860 : 1030 }
        : { left: 800, top: 238, width: 1040, height: 650 };
    const scale = (contain ? Math.min : Math.max)(box.width / w, (box.height - 50) / h);
    const sourceSize = portrait ? [780, 1560] : [1440, 900];
    return (
        <div
            style={{
                position: 'absolute',
                ...box,
                borderRadius: 22,
                overflow: 'hidden',
                background: '#10151c',
                border: '1px solid #4b4430',
                boxShadow: '0 24px 90px #0008',
                opacity: interpolate(frame, [0, 8], [0, 1], ease),
                translate: interpolate(frame, [0, 14], ['0px 20px', '0px 0px'], ease),
            }}
        >
            <div
                style={{
                    height: 50,
                    display: 'flex',
                    alignItems: 'center',
                    gap: 12,
                    padding: '0 20px',
                    borderBottom: `1px solid ${C.line}`,
                    background: '#151a21',
                    fontSize: portrait ? 24 : 21,
                    color: C.muted,
                }}
            >
                <span style={{ width: 8, height: 8, borderRadius: 8, background: C.gold }} />
                {label}
            </div>
            <div style={{ position: 'absolute', top: 50, left: 0, right: 0, bottom: 0, overflow: 'hidden' }}>
                <div
                    style={{
                        position: 'absolute',
                        width: w * scale,
                        height: h * scale,
                        overflow: 'hidden',
                        left: (box.width - w * scale) / 2,
                        top: contain ? (box.height - 50 - h * scale) / 2 : 0,
                    }}
                >
                    <Video
                        name={label}
                        src={staticFile(`footage/${portrait ? 'phone' : 'desktop'}.mp4`)}
                        trimBefore={Math.round(source[portrait ? 1 : 0] * fps)}
                        muted
                        style={{
                            position: 'absolute',
                            width: sourceSize[0] * scale,
                            height: sourceSize[1] * scale,
                            maxWidth: 'none',
                            left: -x * scale,
                            top: -y * scale,
                        }}
                    />
                </div>
            </div>
        </div>
    );
}

function Copy({
    portrait,
    kicker,
    title,
    detail,
    risk = false,
}: {
    portrait: boolean;
    kicker: string;
    title: string;
    detail: string;
    risk?: boolean;
}) {
    const frame = useCurrentFrame();
    return (
        <div
            style={{
                position: 'absolute',
                left: 80,
                top: portrait ? 230 : 245,
                width: portrait ? 920 : 650,
                opacity: interpolate(frame, [0, 7], [0, 1], ease),
                translate: interpolate(frame, [0, 15], ['0px 20px', '0px 0px'], ease),
            }}
        >
            <div
                style={{
                    fontSize: portrait ? 28 : 24,
                    letterSpacing: 3,
                    color: risk ? C.red : C.gold,
                    fontWeight: 650,
                    marginBottom: 22,
                }}
            >
                {kicker}
            </div>
            <div
                style={{
                    fontSize: portrait ? 88 : 91,
                    fontWeight: 760,
                    lineHeight: 1.04,
                    letterSpacing: -4,
                    whiteSpace: 'pre-line',
                }}
            >
                {title}
            </div>
            {!portrait && (
                <div style={{ fontSize: 32, lineHeight: 1.4, color: C.muted, marginTop: 28, maxWidth: 590 }}>
                    {detail}
                </div>
            )}
        </div>
    );
}

function Hook({ portrait }: { portrait: boolean }) {
    const frame = useCurrentFrame();
    return (
        <>
            <Freeze frame={30}>
                <Footage
                    portrait={portrait}
                    source={[40.4, 43.6]}
                    crop={[1035, 100, 400, 250]}
                    phoneCrop={[0, 235, 780, 830]}
                    label="Borrow → sell → locked proceeds · preview"
                    hero
                />
            </Freeze>
            <div style={{ position: 'absolute', left: 80, top: portrait ? 235 : 250, width: portrait ? 920 : 690 }}>
                <div style={{ fontSize: portrait ? 30 : 25, color: C.gold, letterSpacing: 3, marginBottom: 22 }}>
                    ON-CHAIN TOKEN SHORTING
                </div>
                {['Rent the coin.', 'Sell the coin.', 'Buy it back.'].map((line, i) => (
                    <div
                        key={line}
                        style={{
                            fontSize: portrait ? 92 : 94,
                            fontWeight: 780,
                            lineHeight: 1.12,
                            letterSpacing: -4,
                            color: i === 2 ? C.gold : C.text,
                            opacity: interpolate(frame, [i * 15, i * 15 + 10], [0.25, 1], ease),
                            translate: interpolate(frame, [i * 15, i * 15 + 12], ['0px 14px', '0px 0px'], ease),
                        }}
                    >
                        {line}
                    </div>
                ))}
            </div>
        </>
    );
}

function Supply({ portrait }: { portrait: boolean }) {
    const { fps } = useVideoConfig();
    return (
        <>
            <Copy
                portrait={portrait}
                kicker="01 / SUPPLY INVENTORY"
                title={portrait ? 'Supply inventory.' : 'Inventory,\nput to work.'}
                detail="Create a market. Supply tokens for borrowers."
            />
            <Sequence name="Create market · review and sign" durationInFrames={2 * fps}>
                <Footage
                    portrait={portrait}
                    source={[9.2, 8.7]}
                    crop={[420, 275, 600, 375]}
                    phoneCrop={[0, 130, 780, 830]}
                    label="Create market · simulation review"
                />
            </Sequence>
            <Sequence name="Supply tokens · lender disclosure" from={2 * fps} durationInFrames={5 * fps}>
                <Footage
                    portrait={portrait}
                    source={[25.2, 28.2]}
                    crop={[430, 440, 570, 357]}
                    phoneCrop={[0, 730, 780, 830]}
                    label="Deposit · review the repayment risk"
                />
            </Sequence>
        </>
    );
}

function Open({ portrait }: { portrait: boolean }) {
    const { fps } = useVideoConfig();
    return (
        <>
            <Copy
                portrait={portrait}
                kicker="02 / OPEN A SHORT"
                title={portrait ? 'Review. Then sign.' : 'Review it.\nThen sign.'}
                detail="Choose the size. Check collateral and liquidation risk. Sale proceeds stay locked."
            />
            <Sequence name="Choose short size" durationInFrames={3 * fps}>
                <Footage
                    portrait={portrait}
                    source={[39.8, 43.0]}
                    crop={[1038, 110, 395, 247]}
                    phoneCrop={[0, 150, 780, 830]}
                    label="Size · collateral · liquidation price"
                />
            </Sequence>
            <Sequence name="Open short · review collateral" from={3 * fps} durationInFrames={3 * fps}>
                <Footage
                    portrait={portrait}
                    source={[45.2, 49.9]}
                    crop={[420, 100, 600, 375]}
                    phoneCrop={[0, 10, 780, 830]}
                    label="Simulation · review the transaction"
                />
            </Sequence>
            <Sequence name="Open short · sign" from={6 * fps} durationInFrames={6 * fps}>
                <Footage
                    portrait={portrait}
                    source={[48.0, 52.9]}
                    crop={[430, 550, 570, 330]}
                    phoneCrop={[0, 730, 780, 830]}
                    label="Acknowledge the risk · sign in wallet"
                />
            </Sequence>
        </>
    );
}

function Close({ portrait }: { portrait: boolean }) {
    const { fps } = useVideoConfig();
    return (
        <>
            <Copy
                portrait={portrait}
                kicker="03 / BUY BACK TO CLOSE"
                title={portrait ? 'Check the exit.' : 'Buy back.\nClose out.'}
                detail="The preview shows the buyback cost and what returns to the borrower."
            />
            <Sequence name="Close short · preview buyback" durationInFrames={3 * fps}>
                <Footage
                    portrait={portrait}
                    source={[59.5, 65.7]}
                    crop={[420, 185, 600, 375]}
                    phoneCrop={[0, 140, 780, 830]}
                    label="Close preview · buyback cost"
                />
            </Sequence>
            <Sequence name="Close short · sign" from={3 * fps} durationInFrames={1.3 * fps}>
                <Footage
                    portrait={portrait}
                    source={[63.6, 69.8]}
                    crop={[420, 420, 600, 375]}
                    phoneCrop={[0, 730, 780, 830]}
                    label="Sign the buyback"
                />
            </Sequence>
            <Sequence name="Close short · indexed closed status" from={4.3 * fps} durationInFrames={3.7 * fps}>
                <Freeze frame={fps}>
                    <Footage
                        portrait={portrait}
                        source={[68.0, 72.5]}
                        crop={[10, 105, 1400, 310]}
                        phoneCrop={[0, 100, 780, 830]}
                        label="Indexed position · CLOSED · result held"
                        contain={!portrait}
                    />
                </Freeze>
                {!portrait && (
                    <div
                        style={{
                            position: 'absolute',
                            left: 840,
                            top: 330,
                            fontSize: 66,
                            fontWeight: 700,
                            color: '#2ebd85',
                        }}
                    >
                        Closed.
                    </div>
                )}
            </Sequence>
        </>
    );
}

function Exit({ portrait, s1 }: { portrait: boolean; s1: boolean }) {
    const { fps } = useVideoConfig();
    return (
        <>
            <Copy
                portrait={portrait}
                kicker="SEPARATE TEST-ONLY MOCK MARKET"
                title={
                    s1
                        ? portrait
                            ? 'Buyback can fail.'
                            : 'Buyback\ncan fail.'
                        : portrait
                          ? 'Liquidation, recorded.'
                          : 'Liquidation,\nrecorded.'
                }
                detail={
                    s1
                        ? 'Remaining SOL goes to lenders. It may be worth less than the tokens they lent.'
                        : 'A scripted price rise triggers liquidation. The history records the keeper payment.'
                }
                risk={s1}
            />
            <Sequence name={s1 ? 'S1 · settlement record' : 'Liquidation · keeper record'} durationInFrames={4.5 * fps}>
                <Footage
                    portrait={portrait}
                    source={s1 ? [95.2, 104.8] : [79.6, 86.8]}
                    crop={[65, 235, 690, 398]}
                    phoneCrop={[0, 620, 780, 830]}
                    label={s1 ? 'S1 settlement · lenders receive SOL' : 'Liquidation · keeper payment'}
                />
            </Sequence>
            <Sequence name="Hold the recorded result" from={4.5 * fps} durationInFrames={(s1 ? 5.5 : 3.5) * fps}>
                <Freeze frame={4.4 * fps}>
                    <Footage
                        portrait={portrait}
                        source={s1 ? [95.2, 104.8] : [79.6, 86.8]}
                        crop={[65, 235, 690, 398]}
                        phoneCrop={[0, 620, 780, 830]}
                        label={s1 ? 'S1 settlement · result held' : 'Liquidation · result held'}
                    />
                </Freeze>
            </Sequence>
            {!portrait && (
                <div style={{ position: 'absolute', left: 850, top: 520 }}>
                    <div style={{ fontSize: 23, letterSpacing: 2, color: C.muted }}>
                        {s1 ? 'TO LENDERS' : 'KEEPER PAYMENT'}
                    </div>
                    <div
                        style={{
                            fontSize: 76,
                            letterSpacing: -2,
                            fontWeight: 710,
                            marginTop: 8,
                            color: s1 ? C.text : C.gold,
                        }}
                    >
                        {s1 ? '0.8859' : '0.0190'} <span style={{ fontSize: 42, fontWeight: 450 }}>SOL</span>
                    </div>
                    <div style={{ fontSize: 23, color: C.muted, marginTop: 12 }}>Recorded in this test position</div>
                </div>
            )}
            <div
                style={{
                    position: 'absolute',
                    left: portrait ? 104 : 824,
                    top: portrait ? 1396 : 785,
                    width: portrait ? 872 : 992,
                    borderRadius: 14,
                    padding: '20px 26px',
                    background: s1 ? '#321e25f5' : '#20251ff5',
                    border: `1px solid ${s1 ? '#a54e61' : '#56664b'}`,
                    fontSize: portrait ? 32 : 28,
                    fontWeight: 620,
                    lineHeight: 1.25,
                }}
            >
                {s1 ? 'Lenders can receive less than they lent.' : 'Scripted test scenario · separate position'}
            </div>
        </>
    );
}

function End({ portrait }: { portrait: boolean }) {
    const frame = useCurrentFrame();
    return (
        <div
            style={{
                position: 'absolute',
                left: 80,
                right: 80,
                top: portrait ? 375 : 190,
                textAlign: 'center',
                opacity: interpolate(frame, [0, 12], [0, 1], ease),
                translate: interpolate(frame, [0, 18], ['0px 22px', '0px 0px'], ease),
            }}
        >
            <svg
                viewBox="0 0 24 24"
                style={{ width: portrait ? 170 : 100, height: portrait ? 170 : 100, marginBottom: portrait ? 30 : 20 }}
                aria-label="ShortWin"
            >
                <path d="M1 4L4.75 20L8.44 20L12 13.53L12.4 14.25L12.4 7.2L12 6.47L7.14 15.3L4.49 4Z" fill={C.text} />
                <path d="M12 13.53L15.56 20L19.25 20L23 4L19.51 4L16.86 15.3L12 6.47Z" fill={C.gold} />
            </svg>
            <div style={{ fontSize: portrait ? 106 : 96, fontWeight: 780, letterSpacing: -5, lineHeight: 1.04 }}>
                Inspect the code.
                <br />
                <span style={{ color: C.gold }}>Reproduce the demo.</span>
            </div>
            <div
                style={{ color: C.muted, fontSize: portrait ? 39 : 30, lineHeight: 1.5, marginTop: portrait ? 38 : 24 }}
            >
                Watch the full walkthrough.
                <br />
                Follow the build.
            </div>
            <div
                style={{
                    display: 'inline-block',
                    border: `1px solid #665423`,
                    background: '#292416',
                    borderRadius: 14,
                    padding: portrait ? '28px 26px' : '20px 38px',
                    fontSize: portrait ? 36 : 32,
                    marginTop: portrait ? 36 : 24,
                    color: C.gold,
                }}
            >
                github.com/TodorBonchev/short.win
            </div>
            <div style={{ fontSize: portrait ? 30 : 24, color: C.muted, marginTop: portrait ? 28 : 20 }}>
                Captured on a local validator · v1.0.3
            </div>
        </div>
    );
}

function Captions({ portrait, captions }: { portrait: boolean; captions: Caption[] }) {
    const frame = useCurrentFrame();
    const { fps } = useVideoConfig();
    const active = captions.find((c) => (frame / fps) * 1000 >= c.startMs && (frame / fps) * 1000 < c.endMs);
    return (
        <div
            style={{
                position: 'absolute',
                left: portrait ? 84 : 320,
                right: portrait ? 96 : 320,
                top: portrait ? 1564 : 900,
                height: portrait ? 150 : 88,
                display: 'flex',
                alignItems: 'center',
                justifyContent: 'center',
                textAlign: 'center',
                whiteSpace: 'pre-line',
                fontSize: portrait ? 44 : 34,
                fontWeight: 560,
                lineHeight: 1.25,
                color: C.text,
            }}
        >
            {active?.text}
        </div>
    );
}

export function Promo({ portrait, audio = true }: Props) {
    const { fps, durationInFrames } = useVideoConfig();
    const frame = useCurrentFrame();
    const [handle] = useState(() => delayRender('Loading local fonts and narration captions'));
    const [captions, setCaptions] = useState<Caption[]>([]);
    useEffect(() => {
        Promise.all([
            loadFont({ family: 'PromoSans', url: staticFile('fonts/AdwaitaSans.ttf'), weight: '100 900' }),
            fetch(staticFile('audio/captions.json')).then((r) => {
                if (!r.ok) throw new Error('Run pnpm prepare:audio before previewing');
                return r.json();
            }),
        ])
            .then(([, data]) => {
                setCaptions(data);
                continueRender(handle);
            })
            .catch(cancelRender);
    }, [handle]);
    return (
        <AbsoluteFill style={{ fontFamily: 'PromoSans, sans-serif', color: C.text }}>
            <Background />
            <Brand portrait={portrait} />
            <Sequence name="Hook · rent sell buy back" durationInFrames={4 * fps}>
                <Hook portrait={portrait} />
            </Sequence>
            <Sequence name="Supply inventory" from={4 * fps} durationInFrames={7 * fps}>
                <Supply portrait={portrait} />
            </Sequence>
            <Sequence name="Review and open" from={11 * fps} durationInFrames={12 * fps}>
                <Open portrait={portrait} />
            </Sequence>
            <Sequence name="Buy back and close" from={23 * fps} durationInFrames={8 * fps}>
                <Close portrait={portrait} />
            </Sequence>
            <Sequence name="Liquidation · separate mock" from={31 * fps} durationInFrames={8 * fps}>
                <Exit portrait={portrait} s1={false} />
            </Sequence>
            <Sequence name="S1 · separate mock" from={39 * fps} durationInFrames={10 * fps}>
                <Exit portrait={portrait} s1 />
            </Sequence>
            <Sequence name="Explore and reproduce" from={49 * fps} durationInFrames={7 * fps}>
                <End portrait={portrait} />
            </Sequence>
            <Captions portrait={portrait} captions={captions} />
            <div
                style={{
                    position: 'absolute',
                    left: 80,
                    right: 80,
                    bottom: portrait ? 104 : 36,
                    fontSize: portrait ? 28 : 23,
                    color: C.muted,
                    textAlign: portrait ? 'center' : 'left',
                    letterSpacing: 0.6,
                }}
            >
                EDITED LOCAL-VALIDATOR DEMO · v1.0.3 · NOT A DEPLOYMENT
            </div>
            <div
                style={{
                    position: 'absolute',
                    left: 80,
                    bottom: portrait ? 164 : 78,
                    height: 3,
                    width: interpolate(frame, [0, durationInFrames - 1], [0, portrait ? 920 : 1760]),
                    background: C.gold,
                }}
            />
            {audio && <Audio name="Bella narration + original score" src={staticFile('audio/mix.wav')} />}
        </AbsoluteFill>
    );
}
