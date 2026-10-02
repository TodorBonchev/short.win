/**
 * Renders a disclosure from `config/disclosures.ts` verbatim (FRONTEND.md §4). With `ack`, the
 * disclosure's acknowledgement checkbox is shown and its state reported; flows gate signing on it.
 * A disclosure whose acknowledgement is its whole text (SPEC §5.3's sentence) shows the sentence
 * once, as the checkbox's label, rather than twice.
 */
import { DISCLOSURES, type DisclosureId, renderDisclosure } from '../config/disclosures.ts';
import { T } from '../config/testids.ts';
import { Checkbox } from './Field.tsx';

export interface DisclosureProps {
    readonly id: DisclosureId;
    readonly vars?: Readonly<Record<string, string>>;
    /** Acknowledgement state; when given and the disclosure has an `ack` text, a checkbox is shown. */
    readonly acknowledged?: boolean;
    readonly onAcknowledge?: (v: boolean) => void;
    /** Collapsed into `<details>` (for long, non-blocking disclosures). */
    readonly collapsible?: boolean;
}

export function Disclosure({ id, vars, acknowledged, onAcknowledge, collapsible }: DisclosureProps) {
    const d = DISCLOSURES[id];
    const body = renderDisclosure(id, vars);
    const withAck = !!d.ack && !!onAcknowledge;
    const bodyIsAck = withAck && body.length === 1 && body[0] === d.ack;
    const content = (
        <>
            {bodyIsAck ? null : body.length === 1 ? (
                <p>{body[0]}</p>
            ) : (
                <ul>
                    {body.map((p) => (
                        <li key={p}>{p}</li>
                    ))}
                </ul>
            )}
            {withAck && onAcknowledge && (
                <Checkbox checked={!!acknowledged} onChange={onAcknowledge} testId={T.disclosureAck(id)}>
                    {d.ack}
                </Checkbox>
            )}
        </>
    );
    if (collapsible) {
        return (
            <details className="disclosure" data-severity={d.severity} data-testid={T.disclosure(id)}>
                <summary>{d.title}</summary>
                {content}
            </details>
        );
    }
    return (
        <section className="disclosure" data-severity={d.severity} data-testid={T.disclosure(id)} aria-label={d.title}>
            <div className="disclosure-title">{d.title}</div>
            {content}
        </section>
    );
}
