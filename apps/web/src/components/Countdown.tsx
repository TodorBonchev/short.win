import { useEffect, useState } from 'react';
import { formatDuration } from '../lib/format.ts';
import { nowSeconds } from '../lib/time.ts';

/** Time left until `deadline` (unix seconds), ticking every second; "expired" at 0. */
export function Countdown({ deadline, testId }: { deadline: number; testId?: string }) {
    const [now, setNow] = useState(nowSeconds);
    useEffect(() => {
        const t = setInterval(() => setNow(nowSeconds()), 1_000);
        return () => clearInterval(t);
    }, []);
    const left = deadline - now;
    return (
        <span className="num" data-testid={testId} data-value={String(Math.max(0, left))}>
            {left > 0 ? formatDuration(left) : 'expired'}
        </span>
    );
}
