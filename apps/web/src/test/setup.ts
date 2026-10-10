import { cleanup } from '@testing-library/react';
import { afterEach } from 'vitest';
import { resetMedia } from './media.ts';

// jsdom does not implement scrolling; the router's scroll restoration calls it.
window.scrollTo = () => {};

afterEach(() => {
    cleanup();
    resetMedia();
});
