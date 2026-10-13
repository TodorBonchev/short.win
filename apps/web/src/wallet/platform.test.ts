/**
 * Which browser the wallet chooser runs in (FRONTEND.md §5.4 (6), §10.1): a phone or tablet
 * (`isMobileDevice`, unchanged code), a social app's in-app browser (`isInAppBrowser`), and where the
 * Mobile Wallet Adapter registers (`mwaEligible`).
 */
import { afterEach, describe, expect, it } from 'vitest';
import { isMobileDevice } from '../styles/breakpoints.ts';
import { setMedia } from '../test/media.ts';
import { stubNavigator, UA } from '../test/userAgents.ts';
import { isInAppBrowser, mwaEligible } from './platform.ts';

let restore: () => void = () => {};
afterEach(() => restore());

describe('isMobileDevice (a phone or tablet)', () => {
    const cases: [string, Parameters<typeof stubNavigator>[0], boolean][] = [
        ['client hints say mobile', { userAgent: UA.windowsChrome, userAgentData: { mobile: true } }, true],
        ['iPhone', { userAgent: UA.iphoneSafari, maxTouchPoints: 5 }, true],
        ['iPod', { userAgent: UA.ipod, maxTouchPoints: 5 }, true],
        ['an Android phone (Pixel Chrome)', { userAgent: UA.androidChrome, maxTouchPoints: 5 }, true],
        ['an Android tablet (no "Mobile")', { userAgent: UA.androidTablet, maxTouchPoints: 10 }, true],
        ['iPadOS (a Macintosh with 5 touch points)', { userAgent: UA.macSafari, maxTouchPoints: 5 }, true],
        ['an Android WebView', { userAgent: UA.androidWebView, maxTouchPoints: 5 }, true],
        ['macOS Safari (0 touch points)', { userAgent: UA.macSafari, maxTouchPoints: 0 }, false],
        [
            'Windows Chrome with a fine pointer',
            { userAgent: UA.windowsChrome, maxTouchPoints: 0, userAgentData: { mobile: false } },
            false,
        ],
    ];
    it.each(cases)('%s', (_, nav, expected) => {
        restore = stubNavigator(nav);
        expect(isMobileDevice()).toBe(expected);
    });

    it('an unknown user agent with a coarse, non-hovering pointer', () => {
        restore = stubNavigator({ userAgent: 'SomeBrowser/1.0', maxTouchPoints: 0 });
        expect(isMobileDevice()).toBe(false);
        setMedia({ coarse: true });
        expect(isMobileDevice()).toBe(true);
    });
});

describe('isInAppBrowser (a social app or an Android WebView)', () => {
    it.each([
        ['Telegram Android', UA.telegramAndroid],
        ['Instagram', UA.instagramIos],
        ['Facebook iOS', UA.facebookIos],
        ['Facebook Android', UA.facebookAndroid],
        ['Line', UA.lineIos],
        ['X iOS', UA.xIos],
        ['the plain Android WebView', UA.androidWebView],
    ])('%s: yes', (_, ua) => {
        expect(isInAppBrowser(ua)).toBe(true);
    });

    it.each([
        ['Chrome Android', UA.androidChrome],
        ['iOS Safari', UA.iphoneSafari],
        ['desktop Chrome', UA.windowsChrome],
        ['Samsung Internet', UA.samsungInternet],
        ['a "Linux" desktop (the \\bLine\\/ guard)', UA.linuxChrome],
    ])('%s: no', (_, ua) => {
        expect(isInAppBrowser(ua)).toBe(false);
    });
});

describe('mwaEligible (where the Mobile Wallet Adapter registers)', () => {
    it.each([
        ['Chrome Android', UA.androidChrome],
        ['Samsung Internet', UA.samsungInternet],
        ['Brave Android', UA.braveAndroid],
        ['an Android tablet', UA.androidTablet],
        ['the Solana Mobile Web Shell (a WebView, exempt as in the library)', UA.webShell],
    ])('%s in a secure context: yes', (_, ua) => {
        expect(mwaEligible(ua, true)).toBe(true);
    });

    it('nowhere without a secure context', () => {
        expect(mwaEligible(UA.androidChrome, false)).toBe(false);
        expect(mwaEligible(UA.webShell, false)).toBe(false);
    });

    it.each([
        ['iPhone', UA.iphoneSafari],
        ['iPad', UA.macSafari],
        ['desktop Chrome', UA.windowsChrome],
        ['a Linux desktop', UA.linuxChrome],
        ['the Android WebView (; wv)', UA.androidWebView],
        ['an older Android WebView (Version/4.0 Chrome/)', UA.androidWebViewOld],
        ['Telegram on Android (a WebView)', UA.telegramAndroid],
        ['Firefox for Android (FE-34)', UA.firefoxAndroid],
    ])('%s: no', (_, ua) => {
        expect(mwaEligible(ua, true)).toBe(false);
    });
});
