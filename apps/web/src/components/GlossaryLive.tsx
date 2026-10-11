/**
 * A glossary definition with placeholders, filled from the live `GlobalConfig` (`{vestPeriod}`,
 * `{lockGrace}`). Its own chunk, loaded when such a definition opens: the ⓘ sits in the app shell
 * (the status bar), and the chain-account readers behind `useChainConfig` stay out of the first load.
 */
import type { GlossaryId } from '../config/glossary.ts';
import { glossaryText, glossaryVars } from '../config/glossaryText.ts';
import { useChainConfig } from '../rpc/queries.ts';

export default function GlossaryLive({ id }: { id: GlossaryId }) {
    const config = useChainConfig();
    const params = config.data?.exists ? config.data.data.params : null;
    return <>{glossaryText(id, glossaryVars(params))}</>;
}
