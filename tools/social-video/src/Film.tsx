import React from 'react';
import {Audio} from '@remotion/media';
import {TransitionSeries, linearTiming} from '@remotion/transitions';
import {fade} from '@remotion/transitions/fade';
import {loadFont} from '@remotion/fonts';
import {AbsoluteFill, CanvasImage, Easing, getStaticFiles, interpolate, staticFile, useCurrentFrame, useVideoConfig} from 'remotion';

loadFont({family: 'Film Sans', url: staticFile('fonts/NotoSans-Regular.ttf'), weight: '400'});
loadFont({family: 'Film Sans', url: staticFile('fonts/NotoSans-Medium.ttf'), weight: '500'});
loadFont({family: 'Film Serif', url: staticFile('fonts/NotoSerif-Italic.ttf'), style: 'italic'});

const background = '#0b0e11';
const panel = '#12161b';
const raised = '#181d23';
const white = '#e8ecf1';
const muted = '#aab4bf';
const orange = '#f0b90b';
const ease = Easing.bezier(0.16, 1, 0.3, 1);
const clamp = {extrapolateLeft: 'clamp', extrapolateRight: 'clamp'} as const;

const Mark = ({size = 34}: {size?: number}) => (
  <svg width={size} height={size} viewBox="0 0 24 24" fill="none" aria-label="short.win W">
    <path d="M1 4L4.75 20L8.44 20L12 13.53L12.4 14.25L12.4 7.2L12 6.47L7.14 15.3L4.49 4Z" fill={white} />
    <path d="M12 13.53L15.56 20L19.25 20L23 4L19.51 4L16.86 15.3L12 6.47Z" fill={orange} />
  </svg>
);

const Background = () => {
  const frame = useCurrentFrame();
  const {durationInFrames} = useVideoConfig();
  return <AbsoluteFill style={{background: background, overflow: 'hidden'}}>
    <div style={{position: 'absolute', width: 1500, height: 1500, left: -700, top: 250, borderRadius: '50%', background: 'radial-gradient(ellipse, #f0b90b12, transparent 65%)', translate: interpolate(frame, [0, durationInFrames], ['0px 0px', '140px -90px'], clamp)}} />
    <div style={{position: 'absolute', width: 1200, height: 1200, right: -650, bottom: -260, borderRadius: '50%', background: 'radial-gradient(ellipse, #aab4bf12, transparent 68%)'}} />
    <div style={{position: 'absolute', inset: 0, opacity: .12, backgroundImage: 'linear-gradient(#aab4bf30 1px, transparent 1px), linear-gradient(90deg, #aab4bf30 1px, transparent 1px)', backgroundSize: '120px 120px', maskImage: 'linear-gradient(transparent, black 45%, transparent)'}} />
  </AbsoluteFill>;
};

const Brand = () => {
  const {width} = useVideoConfig();
  const wide = width > 1200;
  return <>
    <div style={{position: 'absolute', left: wide ? 100 : 84, top: wide ? 66 : 100, display: 'flex', alignItems: 'center', gap: 13, color: white, fontFamily: 'Film Sans', fontSize: wide ? 44 : 44, fontWeight: 500, letterSpacing: '-1.5px'}}><Mark size={wide ? 33 : 37} /><span>short<span style={{color: orange}}>.win</span></span></div>
    <div style={{position: 'absolute', right: wide ? 100 : 84, top: wide ? 73 : 112, color: white, fontSize: wide ? 19 : 21, fontFamily: 'Film Sans', fontWeight: 500, letterSpacing: '2px', display: 'flex', alignItems: 'center', gap: 12}}><span style={{height: 7, width: 7, borderRadius: 99, background: orange, boxShadow: '0 0 16px #f0b90b66'}} />LOCAL FORK · TEST FUNDS</div>
  </>;
};

const Capture = ({asset}: {asset: string}) => {
  const exists = getStaticFiles().some((file) => file.name === `captures/${asset}.jpg`);
  if (!exists) return <div style={{width: '100%', aspectRatio: '390 / 844', display: 'flex', flexDirection: 'column', background: panel, color: muted, alignItems: 'center', justifyContent: 'center', textAlign: 'center', fontFamily: 'Film Sans', gap: 24, border: '1px dashed #aab4bf66'}}><Mark size={60} /><div style={{fontSize: 30}}>CAPTURE PENDING</div><div style={{fontSize: 22}}>{asset}.jpg</div></div>;
  return <CanvasImage src={staticFile(`captures/${asset}.jpg`)} style={{width: '100%', height: 'auto', display: 'block'}} />;
};

const Phone = ({asset, width = 590, angle = -.6, top, left, subtle = false}: {asset: string; width?: number; angle?: number; top: number; left: number; subtle?: boolean}) => {
  const frame = useCurrentFrame();
  const {durationInFrames, fps} = useVideoConfig();
  return <div style={{position: 'absolute', width, top, left, padding: 10, boxSizing: 'border-box', borderRadius: 58, background: 'linear-gradient(135deg, #aab4bf80, #181d23 25%, #12161b 80%, #aab4bf66)', border: '1px solid #aab4bf70', boxShadow: subtle ? '0 35px 90px #0007' : '0 45px 115px #0009, 0 0 90px #f0b90b0b', rotate: `${angle}deg`, scale: interpolate(frame, [0, durationInFrames - 1], [.985, 1.025], {...clamp, output: 'perceptual-scale'}), translate: interpolate(frame, [0, .8 * fps], ['0px 35px', '0px 0px'], {...clamp, easing: ease})}}>
    <div style={{width: '100%', borderRadius: 48, overflow: 'hidden', position: 'relative', background: background, border: '1px solid #0b0e11'}}><Capture asset={asset} /></div>
    <div style={{position: 'absolute', right: -4, top: 172, height: 70, width: 4, borderRadius: '0 3px 3px 0', background: muted}} />
  </div>;
};

const Progress = ({step, total = 3}: {step: number; total?: number}) => <div style={{display: 'flex', gap: 10}}>{Array.from({length: total}, (_, index) => <div key={index} style={{width: 47, height: 3, background: index < step ? orange : raised, opacity: index < step ? 1 : .6}} />)}</div>;

type ShotProps = {asset: string; eyebrow: string; title: string; accent: string; detail: string; step: number; total?: number; footnote?: string; angle?: number};
export const WorkflowShot = ({asset, eyebrow, title, accent, detail, step, total = 3, footnote, angle = -.6}: ShotProps) => {
  const frame = useCurrentFrame();
  const {width, fps} = useVideoConfig();
  const wide = width > 1200;
  return <AbsoluteFill style={{color: white, fontFamily: 'Film Sans', overflow: 'hidden'}}>
    <div style={{position: 'absolute', left: wide ? 112 : 88, top: wide ? 250 : 226, width: wide ? 820 : 904, opacity: interpolate(frame, [0, .55 * fps], [0, 1], clamp), translate: interpolate(frame, [0, .7 * fps], ['0px 28px', '0px 0px'], {...clamp, easing: ease})}}>
      <div style={{fontSize: wide ? 24 : 27, fontWeight: 500, color: orange, letterSpacing: '4px', textTransform: 'uppercase', marginBottom: wide ? 34 : 26}}>{eyebrow}</div>
      <h1 style={{fontSize: wide ? 98 : 88, fontWeight: 400, letterSpacing: wide ? '-5px' : '-4px', lineHeight: 1.08, margin: 0, whiteSpace: 'pre-line'}}>{title}<br /><span style={{fontFamily: 'Film Serif', fontStyle: 'italic', color: orange, letterSpacing: '-4px'}}>{accent}</span></h1>
      <div style={{fontSize: wide ? 44 : 44, lineHeight: 1.45, letterSpacing: '-.8px', color: muted, marginTop: wide ? 30 : 26, width: wide ? 700 : 900}}>{detail}</div>
      {wide ? <div style={{marginTop: 48}}><Progress step={step} total={total} /></div> : null}
      {wide && footnote ? <div style={{fontSize: 24, color: muted, width: 700, lineHeight: 1.55, marginTop: 32}}>{footnote}</div> : null}
    </div>
    <Phone asset={asset} width={wide ? 400 : 520} top={wide ? 156 : 590} left={wide ? 1180 : 280} angle={angle} />
    {!wide ? <div style={{position: 'absolute', left: 88, right: 88, bottom: 83, display: 'flex', alignItems: 'center', justifyContent: 'space-between'}}><Progress step={step} total={total} /><span style={{color: muted, fontSize: 21, letterSpacing: '1.2px'}}>ACTUAL APP · LOCAL DEMO</span></div> : null}
  </AbsoluteFill>;
};

const Intro = ({section, title, accent, asset = 'workspace', description}: {section: string; title: string; accent: string; asset?: string; description: string}) => {
  const frame = useCurrentFrame();
  const {width, fps} = useVideoConfig();
  const wide = width > 1200;
  return <AbsoluteFill style={{fontFamily: 'Film Sans', color: white, overflow: 'hidden'}}>
    <div style={{position: 'absolute', width: wide ? 840 : 900, left: wide ? 112 : 88, top: wide ? 276 : 345, opacity: interpolate(frame, [0, .5 * fps], [0, 1], clamp), translate: interpolate(frame, [0, .7 * fps], ['0px 35px', '0px 0px'], {...clamp, easing: ease})}}>
      <div style={{fontSize: wide ? 26 : 29, color: orange, letterSpacing: '5px', fontWeight: 500, textTransform: 'uppercase', marginBottom: 36}}>{section}</div>
      <h1 style={{fontSize: wide ? 116 : 110, fontWeight: 400, lineHeight: 1.05, letterSpacing: '-6px', margin: 0}}>{title}<br /><span style={{fontFamily: 'Film Serif', fontStyle: 'italic', color: orange}}>{accent}</span></h1>
      <div style={{fontSize: wide ? 36 : 44, color: muted, lineHeight: 1.5, marginTop: 36, maxWidth: 770}}>{description}</div>
      <div style={{width: 70, height: 2, background: orange, marginTop: 44}} />
    </div>
    <Phone asset={asset} width={wide ? 400 : 520} top={wide ? 154 : 940} left={wide ? 1180 : 280} angle={wide ? 2 : 3} subtle />
    {!wide ? <div style={{position: 'absolute', inset: 0, background: 'linear-gradient(transparent 82%, #0b0e11)', pointerEvents: 'none'}} /> : null}
  </AbsoluteFill>;
};

const EndCard = ({section, note}: {section: string; note: string}) => {
  const frame = useCurrentFrame();
  const {width, fps} = useVideoConfig();
  const wide = width > 1200;
  return <AbsoluteFill style={{fontFamily: 'Film Sans', color: white, alignItems: 'center', justifyContent: 'center', textAlign: 'center', opacity: interpolate(frame, [0, .45 * fps], [0, 1], clamp)}}>
    <div style={{width: wide ? 1450 : 900, translate: interpolate(frame, [0, .7 * fps], ['0px 24px', '0px 0px'], {...clamp, easing: ease})}}>
      <div style={{display: 'flex', justifyContent: 'center', marginBottom: 40}}><Mark size={wide ? 70 : 86} /></div>
      <div style={{fontSize: wide ? 25 : 29, fontWeight: 500, color: orange, letterSpacing: '5px', textTransform: 'uppercase', marginBottom: 30}}>{section}</div>
      <h1 style={{fontSize: wide ? 110 : 99, fontWeight: 400, lineHeight: 1.1, letterSpacing: '-5px', margin: 0}}>Choose your side.<br /><span style={{fontFamily: 'Film Serif', fontStyle: 'italic', color: orange}}>Know your terms.</span></h1>
      <div style={{fontSize: wide ? 32 : 36, color: white, maxWidth: wide ? 1160 : 850, margin: '38px auto 0', lineHeight: 1.5}}>{note}</div>
      <div style={{width: 80, height: 1, background: muted, margin: '48px auto 34px'}} />
      <div style={{fontSize: wide ? 28 : 31, color: muted, lineHeight: 1.65}}>Synthetic price history.<br />Historical fork state. Experimental software.</div>
      <div style={{fontSize: wide ? 23 : 26, color: muted, marginTop: 30}}>Local execution only · No public funds</div>
    </div>
  </AbsoluteFill>;
};

const FilmBase = ({children}: {children: React.ReactNode}) => {
  const frame = useCurrentFrame();
  const {durationInFrames, fps} = useVideoConfig();
  return <AbsoluteFill><Background />{children}<Brand />
    <Audio name="Original synthesized score" src={staticFile('audio/original-score.m4a')} durationInFrames={durationInFrames} volume={interpolate(frame, [0, .7 * fps, durationInFrames - 1.4 * fps, durationInFrames - 1], [0, .65, .65, 0], clamp)} />
  </AbsoluteFill>;
};

export const LendFilm = () => {
  const {fps} = useVideoConfig();
  return <FilmBase><TransitionSeries name="Lend workflow">
    <TransitionSeries.Sequence name="Lend introduction" durationInFrames={3 * fps}><Intro section="01 / Lend" title="Put tokens" accent="to work." description="Supply liquidity. See your share." /></TransitionSeries.Sequence>
    <TransitionSeries.Transition presentation={fade()} timing={linearTiming({durationInFrames: 10})} />
    <TransitionSeries.Sequence name="Choose lending amount" durationInFrames={6 * fps}><WorkflowShot asset="lend-form" eyebrow="01 / Supply" title="Your tokens." accent="Your amount." detail="Choose how much to lend." step={1} /></TransitionSeries.Sequence>
    <TransitionSeries.Transition presentation={fade()} timing={linearTiming({durationInFrames: 10})} />
    <TransitionSeries.Sequence name="Review deposit" durationInFrames={6 * fps}><WorkflowShot asset="lend-review" eyebrow="02 / Review" title="Clear terms." accent="Your approval." detail="Review the deposit before signing." step={2} angle={.6} /></TransitionSeries.Sequence>
    <TransitionSeries.Transition presentation={fade()} timing={linearTiming({durationInFrames: 10})} />
    <TransitionSeries.Sequence name="Lending confirmation" durationInFrames={6 * fps}><WorkflowShot asset="lend-success" eyebrow="03 / Receive shares" title="Your pool share," accent="in view." detail="Track shares and paid SOL fees." step={3} /></TransitionSeries.Sequence>
    <TransitionSeries.Transition presentation={fade()} timing={linearTiming({durationInFrames: 10})} />
    <TransitionSeries.Sequence name="Lending disclosure" durationInFrames={130}><EndCard section="Lend / short.win" note="Fees depend on borrower payments. Withdrawals depend on available liquidity." /></TransitionSeries.Sequence>
  </TransitionSeries></FilmBase>;
};

export const ShortFilm = () => {
  const {fps} = useVideoConfig();
  return <FilmBase><TransitionSeries name="Short workflow">
    <TransitionSeries.Sequence name="Short introduction" durationInFrames={3 * fps}><Intro section="02 / Short" title="Take the" accent="other side." description="Borrow. Sell. Know your limits." asset="short-form" /></TransitionSeries.Sequence>
    <TransitionSeries.Transition presentation={fade()} timing={linearTiming({durationInFrames: 10})} />
    <TransitionSeries.Sequence name="Set quantity and spending limits" durationInFrames={200}><WorkflowShot asset="short-form" eyebrow="01 / Set limits" title="Choose quantity." accent="Set your limits." detail="Define collateral and spending caps." step={1} /></TransitionSeries.Sequence>
    <TransitionSeries.Transition presentation={fade()} timing={linearTiming({durationInFrames: 10})} />
    <TransitionSeries.Sequence name="Review short terms" durationInFrames={200}><WorkflowShot asset="short-review" eyebrow="02 / Review" title="Every term," accent="before signing." detail="Review fee and SOL collateral." step={2} angle={.6} /></TransitionSeries.Sequence>
    <TransitionSeries.Transition presentation={fade()} timing={linearTiming({durationInFrames: 10})} />
    <TransitionSeries.Sequence name="Local Jupiter short execution" durationInFrames={230}><WorkflowShot asset="short-success" eyebrow="03 / Open position" title="Borrowed. Sold." accent="Through Jupiter." detail="Genuine program execution, locally." step={3} /></TransitionSeries.Sequence>
    <TransitionSeries.Transition presentation={fade()} timing={linearTiming({durationInFrames: 10})} />
    <TransitionSeries.Sequence name="Short disclosure" durationInFrames={160}><EndCard section="Short / short.win" note="Test funds only. The fixed upfront term fee is not refunded on early close." /></TransitionSeries.Sequence>
  </TransitionSeries></FilmBase>;
};

export const ManageFilm = () => {
  const {fps} = useVideoConfig();
  return <FilmBase><TransitionSeries name="Manage and close workflow">
    <TransitionSeries.Sequence name="Manage introduction" durationInFrames={3 * fps}><Intro section="03 / Manage & close" title="Stay close to" accent="your position." description="Collateral, repayment, and a clear exit." asset="position" /></TransitionSeries.Sequence>
    <TransitionSeries.Transition presentation={fade()} timing={linearTiming({durationInFrames: 10})} />
    <TransitionSeries.Sequence name="Read position" durationInFrames={160}><WorkflowShot asset="position" eyebrow="01 / Keep track" title="Your position," accent="at a glance." detail="See the loan and its terms." step={1} total={4} /></TransitionSeries.Sequence>
    <TransitionSeries.Transition presentation={fade()} timing={linearTiming({durationInFrames: 10})} />
    <TransitionSeries.Sequence name="Add collateral" durationInFrames={160}><WorkflowShot asset="margin" eyebrow="02 / Add collateral" title="More collateral." accent="On your terms." detail="Add SOL to the position." step={2} total={4} angle={.6} /></TransitionSeries.Sequence>
    <TransitionSeries.Transition presentation={fade()} timing={linearTiming({durationInFrames: 10})} />
    <TransitionSeries.Sequence name="Review full close" durationInFrames={6 * fps}><WorkflowShot asset="close-review" eyebrow="03 / Review buyback" title="Set a cap." accent="Review the close." detail="Buy back the full borrowed quantity." step={3} total={4} /></TransitionSeries.Sequence>
    <TransitionSeries.Transition presentation={fade()} timing={linearTiming({durationInFrames: 10})} />
    <TransitionSeries.Sequence name="Closed position confirmation" durationInFrames={170}><WorkflowShot asset="close-success" eyebrow="04 / Full repayment" title="Debt repaid." accent="Position closed." detail="Confirmed on the local fork." step={4} total={4} angle={.6} /></TransitionSeries.Sequence>
    <TransitionSeries.Transition presentation={fade()} timing={linearTiming({durationInFrames: 10})} />
    <TransitionSeries.Sequence name="Manage disclosure" durationInFrames={130}><EndCard section="Manage & close / short.win" note="A close requires every borrowed token. Early close does not refund the term fee." /></TransitionSeries.Sequence>
  </TransitionSeries></FilmBase>;
};

export const OverviewFilm = () => {
  const {fps} = useVideoConfig();
  return <FilmBase><TransitionSeries name="short.win overview">
    <TransitionSeries.Sequence name="Overview introduction" durationInFrames={4 * fps}><Intro section="A different side of the market" title="Put tokens" accent="to work." description="Lend, short, and manage positions with clear terms." /></TransitionSeries.Sequence>
    <TransitionSeries.Transition presentation={fade()} timing={linearTiming({durationInFrames: 12})} />
    <TransitionSeries.Sequence name="Overview lend" durationInFrames={8 * fps}><WorkflowShot asset="lend-form" eyebrow="01 / Lend" title="Supply tokens." accent="Receive shares." detail="Choose your amount and supply liquidity to the pool." step={1} total={4} footnote="Lenders share in SOL fees actually paid by borrowers. Withdrawals depend on available liquidity." /></TransitionSeries.Sequence>
    <TransitionSeries.Transition presentation={fade()} timing={linearTiming({durationInFrames: 12})} />
    <TransitionSeries.Sequence name="Overview deposit review" durationInFrames={5 * fps}><WorkflowShot asset="lend-review" eyebrow="Your wallet / Your approval" title="Review first." accent="Then approve." detail="Read the transaction details before signing." step={1} total={4} angle={.6} /></TransitionSeries.Sequence>
    <TransitionSeries.Transition presentation={fade()} timing={linearTiming({durationInFrames: 12})} />
    <TransitionSeries.Sequence name="Overview short" durationInFrames={8 * fps}><WorkflowShot asset="short-form" eyebrow="02 / Short" title="Choose quantity." accent="Set your limits." detail="Borrow and sell through Jupiter with SOL collateral." step={2} total={4} footnote="This walkthrough executes the genuine Jupiter program on a local historical fork, using test funds." /></TransitionSeries.Sequence>
    <TransitionSeries.Transition presentation={fade()} timing={linearTiming({durationInFrames: 12})} />
    <TransitionSeries.Sequence name="Overview short confirmation" durationInFrames={5 * fps}><WorkflowShot asset="short-success" eyebrow="Local program execution" title="Borrowed. Sold." accent="Position open." detail="A fixed term and upfront fee, shown before approval." step={2} total={4} angle={.6} footnote="Closing early does not refund the upfront term fee." /></TransitionSeries.Sequence>
    <TransitionSeries.Transition presentation={fade()} timing={linearTiming({durationInFrames: 12})} />
    <TransitionSeries.Sequence name="Overview manage" durationInFrames={7 * fps}><WorkflowShot asset="position" eyebrow="03 / Manage" title="Your position," accent="in view." detail="Read the loan, add SOL collateral, or prepare repayment." step={3} total={4} /></TransitionSeries.Sequence>
    <TransitionSeries.Transition presentation={fade()} timing={linearTiming({durationInFrames: 12})} />
    <TransitionSeries.Sequence name="Overview close" durationInFrames={6 * fps}><WorkflowShot asset="close-success" eyebrow="04 / Close" title="Full repayment." accent="A clear close." detail="Buy back and repay the full borrowed quantity." step={4} total={4} angle={.6} /></TransitionSeries.Sequence>
    <TransitionSeries.Transition presentation={fade()} timing={linearTiming({durationInFrames: 12})} />
    <TransitionSeries.Sequence name="Overview disclosure" durationInFrames={144}><EndCard section="short.win" note="Local historical fork · Test funds · No public-network trade shown" /></TransitionSeries.Sequence>
  </TransitionSeries></FilmBase>;
};

export const Poster = ({kind}: {kind: 'lend' | 'short' | 'manage'}) => {
  const asset = kind === 'lend' ? 'lend-form' : kind === 'short' ? 'short-form' : 'position';
  return <AbsoluteFill style={{fontFamily: 'Film Sans', color: white}}><Background /><Brand />
    <div style={{position: 'absolute', left: 88, top: 270, right: 88}}>
      <div style={{fontSize: 29, letterSpacing: '4px', color: orange, marginBottom: 28}}>{kind === 'lend' ? '01 / LEND' : kind === 'short' ? '02 / SHORT' : '03 / MANAGE & CLOSE'}</div>
      <div style={{fontSize: 100, lineHeight: 1.08, letterSpacing: '-5px'}}>{kind === 'lend' ? 'Put tokens' : kind === 'short' ? 'Take the' : 'Stay close to'}<br /><span style={{fontFamily: 'Film Serif', fontStyle: 'italic', color: orange}}>{kind === 'lend' ? 'to work.' : kind === 'short' ? 'other side.' : 'your position.'}</span></div>
    </div>
    <div style={{position: 'absolute', left: 290, top: 660, width: 500, padding: 10, background: raised, border: '1px solid #aab4bf70', borderRadius: 58, boxShadow: '0 45px 115px #0009', overflow: 'hidden'}}><div style={{borderRadius: 48, overflow: 'hidden'}}><Capture asset={asset} /></div></div>
    <div style={{position: 'absolute', left: 88, right: 88, bottom: 100, fontSize: 35, color: muted}}>Choose your side. Know your terms.</div>
  </AbsoluteFill>;
};
