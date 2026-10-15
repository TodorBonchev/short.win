/**
 * The vulnerability disclosure policy (`/security`; M10 Q3, SECTXT-4): the page the `Policy` of
 * `/.well-known/security.txt` and the programs' embedded `security.txt` point to. Its text is the
 * repository's `SECURITY.md` (scope, how to report, what happens next, safe harbour, audits); the
 * values come from `config/securityPolicy.ts`. Static: no chain or indexer read. Test ids: the page root
 * (`page-security`) and the status bar's `nav-security` link; each section is a region named by its
 * title (the E2E selects by role). Centred at 920 px on a wide screen; full width on a phone, where
 * the long identifiers and the address wrap anywhere (M10-RESP).
 */
import { Link } from '@tanstack/react-router';
import { type Column, DataTable } from '../../components/DataTable.tsx';
import { Panel } from '../../components/Panel.tsx';
import { SHORTWIN_PROGRAM } from '../../config/programs.ts';
import { SECURITY_EMAIL, SECURITY_TXT_CANONICAL } from '../../config/securityPolicy.ts';
import { SECURITY_TXT_PATH } from '../../config/securityTxt.ts';
import { T } from '../../config/testids.ts';
import './security.css';

const SCOPE: readonly (readonly [string, string])[] = [
    ['short.win program (Solana mainnet)', SHORTWIN_PROGRAM],
    ['Web app', 'https://short.win'],
    ['Keeper, indexer and API operated by short.win', 'in scope'],
];

const SCOPE_COLUMNS: readonly Column<readonly [string, string]>[] = [
    { key: 'what', header: 'Component', card: 'title', render: ([what]) => what },
    { key: 'id', header: 'Identifier', render: ([, id]) => <span className="num sec-id">{id}</span> },
];

export function SecurityPolicyPage() {
    return (
        <div className="page sec-page" data-testid={T.page('security')}>
            <div className="page-head">
                <h1>Security policy</h1>
            </div>
            <p className="sec-lede">
                Found a vulnerability in short.win? Report it privately to{' '}
                <a href={`mailto:${SECURITY_EMAIL}`}>{SECURITY_EMAIL}</a>. Please do not open a public issue, pull
                request or discussion for it.
            </p>
            <div className="stack">
                <Panel title="Scope" label="Scope">
                    <DataTable
                        caption="Components in scope"
                        className="mk-tbl sec-scope"
                        columns={SCOPE_COLUMNS}
                        rows={SCOPE}
                        rowKey={([what]) => what}
                    />
                    <p className="sec-note">
                        The program embeds a security.txt; read it with{' '}
                        <code>query-security-txt &lt;program id&gt;</code>.
                    </p>
                    <h3>Not in scope</h3>
                    <ul className="sec-list">
                        <li>
                            The third-party programs short.win calls: pump.fun, PumpSwap, pump-fees, Raydium, SPL Token
                            / Token-2022, Pyth and Squads. Report issues in them to their maintainers.
                        </li>
                        <li>The test-only mock venue, which is never deployed.</li>
                        <li>
                            The issues already listed, with their bounds, in the audit package's known-issues register,
                            unless you show that a stated bound is wrong or can be exceeded.
                        </li>
                    </ul>
                </Panel>

                <Panel title="Reporting a vulnerability" label="Reporting a vulnerability">
                    <ul className="sec-list">
                        <li>
                            <strong>Contact:</strong> <a href={`mailto:${SECURITY_EMAIL}`}>{SECURITY_EMAIL}</a>
                        </li>
                        <li>
                            <strong>Encryption:</strong> none published yet. If your report needs an encrypted channel,
                            say so in a first short message without the details and we will arrange one.
                        </li>
                        <li>
                            <strong>Language:</strong> English.
                        </li>
                    </ul>
                    <h3>Please include</h3>
                    <ul className="sec-list">
                        <li>the affected program or component, and the commit or deployed program hash;</li>
                        <li>
                            the impact: which funds, or which liveness property (exits, withdrawals, claims), and how
                            much;
                        </li>
                        <li>a reproducer (a LiteSVM test is ideal);</li>
                        <li>whether the issue is being exploited, as far as you know.</li>
                    </ul>
                </Panel>

                <Panel title="What happens next" label="What happens next">
                    <ul className="sec-list">
                        <li>
                            <strong>Acknowledgement and triage:</strong> best effort. We acknowledge a report as soon as
                            we can and keep the reporter informed of the triage and the fix.
                        </li>
                        <li>
                            <strong>Fix:</strong> a fix that needs a program upgrade goes through the Governance
                            multisig's timelock. Before the upgrade, the Emergency multisig can pause new opens and
                            disable a venue for opens. No admin key can move user funds or pause exits.
                        </li>
                        <li>
                            <strong>Disclosure:</strong> coordinated, on a best-effort timeline agreed with the
                            reporter. We publish an advisory after the fix is deployed, crediting the reporter unless
                            they ask otherwise.
                        </li>
                        <li>
                            <strong>Rewards:</strong> no bug bounty yet. A bounty programme is planned for the beta,
                            after the audit and the public audit contest; its terms will be published here.
                        </li>
                    </ul>
                </Panel>

                <Panel title="Safe harbour" label="Safe harbour">
                    <p>
                        We will not pursue or support legal action against research done in good faith under this
                        policy: research that
                    </p>
                    <ul className="sec-list">
                        <li>
                            avoids privacy violations, the loss or destruction of data and the interruption or
                            degradation of the service;
                        </li>
                        <li>
                            never moves or puts at risk funds that are not your own (reproduce on a local validator or a
                            LiteSVM fork, never against other users on mainnet);
                        </li>
                        <li>does not exploit an issue beyond what is needed to show it;</li>
                        <li>reports the issue to us promptly and keeps it confidential until a fix is deployed.</li>
                    </ul>
                </Panel>

                <Panel title="Audits" label="Audits">
                    <p>
                        No audit has been published yet: a full private audit, then a public audit contest, is planned
                        before mainnet. Reports will be listed on this page once published.
                    </p>
                </Panel>

                <p className="sec-note">
                    Machine-readable: <a href={SECURITY_TXT_PATH}>{SECURITY_TXT_CANONICAL}</a> (RFC 9116).{' '}
                    <Link to="/markets">Back to markets</Link>
                </p>
            </div>
        </div>
    );
}
