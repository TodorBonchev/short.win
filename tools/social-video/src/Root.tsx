import {Composition, Folder, Still} from 'remotion';
import {LendFilm, ShortFilm, ManageFilm, OverviewFilm, Poster} from './Film';

export const RemotionRoot = () => <>
  <Folder name="Social-vertical">
    <Composition id="Lend" component={LendFilm} width={1080} height={1920} fps={30} durationInFrames={720} />
    <Composition id="Short" component={ShortFilm} width={1080} height={1920} fps={30} durationInFrames={840} />
    <Composition id="ManageClose" component={ManageFilm} width={1080} height={1920} fps={30} durationInFrames={840} />
  </Folder>
  <Folder name="Overview-horizontal">
    <Composition id="Overview" component={OverviewFilm} width={1920} height={1080} fps={30} durationInFrames={1350} />
  </Folder>
  <Folder name="Posters">
    <Still id="LendPoster" component={Poster} width={1080} height={1920} defaultProps={{kind: 'lend'}} />
    <Still id="ShortPoster" component={Poster} width={1080} height={1920} defaultProps={{kind: 'short'}} />
    <Still id="ManagePoster" component={Poster} width={1080} height={1920} defaultProps={{kind: 'manage'}} />
  </Folder>
</>;
