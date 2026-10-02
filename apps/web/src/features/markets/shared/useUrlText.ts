/**
 * A text input bound to a search parameter: typing writes the normalised value to the URL after
 * `delayMs` (history replace), and a URL change made elsewhere (Back, a link, the header search)
 * replaces the text. The URL never overwrites what the user is typing, and the text never undoes a
 * navigation.
 */
import { useEffect, useRef, useState } from 'react';

function useDebounced<V>(v: V, ms: number): V {
    const [x, setX] = useState(v);
    useEffect(() => {
        const t = setTimeout(() => setX(v), ms);
        return () => clearTimeout(t);
    }, [v, ms]);
    return x;
}

export function useUrlText(
    urlValue: string,
    push: (value: string) => void,
    normalize: (text: string) => string,
    delayMs = 250,
): [string, (text: string) => void] {
    const [text, setText] = useState(urlValue);
    const debounced = useDebounced(normalize(text), delayMs);
    const pushed = useRef(urlValue);
    const url = useRef(urlValue);
    url.current = urlValue;
    const pushFn = useRef(push);
    pushFn.current = push;

    useEffect(() => {
        if (debounced !== url.current) {
            pushed.current = debounced;
            pushFn.current(debounced);
        }
    }, [debounced]);

    useEffect(() => {
        if (urlValue !== pushed.current) {
            pushed.current = urlValue;
            setText(urlValue);
        }
    }, [urlValue]);

    return [text, setText];
}
