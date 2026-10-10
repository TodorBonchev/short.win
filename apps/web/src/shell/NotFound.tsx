import { Link } from '@tanstack/react-router';
import { Empty } from '../components/States.tsx';
import { T } from '../config/testids.ts';

/** An unknown route or an invalid address in one (FRONTEND.md §3.10): a way back, not a dead end. */
export function NotFound() {
    return (
        <div className="page">
            <Empty
                role="alert"
                title="Not found"
                action={
                    <Link to="/markets" className="btn" data-variant="primary" data-testid={T.notFoundCta}>
                        Browse markets
                    </Link>
                }
            >
                There is nothing at this address.
            </Empty>
        </div>
    );
}
