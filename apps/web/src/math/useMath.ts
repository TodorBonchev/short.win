import { useQuery } from '@tanstack/react-query';
import { loadMath, type MathModule } from './load.ts';

/** The math module once loaded (`undefined` while loading; previews show skeletons meanwhile). */
export function useMath(): { math: MathModule | undefined; error: unknown } {
    const q = useQuery({
        queryKey: ['wasm', 'shortwin-math'],
        queryFn: loadMath,
        staleTime: Number.POSITIVE_INFINITY,
        gcTime: Number.POSITIVE_INFINITY,
        structuralSharing: false,
        retry: 1,
    });
    return { math: q.data, error: q.error };
}
