/**
 * User agents for the wallet tests (FRONTEND.md §5.4 (6), §10.1), as the browsers send them, and a
 * way to stub `navigator` with one. Chrome's reduced UA (`Chrome/<major>.0.0.0`) is not a WebView;
 * an Android WebView carries `; wv)` and a full `Version/4.0 Chrome/a.b.c.d`.
 */

export const UA = {
    androidChrome:
        'Mozilla/5.0 (Linux; Android 14; Pixel 7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/141.0.0.0 Mobile Safari/537.36',
    androidTablet:
        'Mozilla/5.0 (Linux; Android 14; SM-X710) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/141.0.0.0 Safari/537.36',
    samsungInternet:
        'Mozilla/5.0 (Linux; Android 14; SM-S918B) AppleWebKit/537.36 (KHTML, like Gecko) SamsungBrowser/28.0 Chrome/130.0.0.0 Mobile Safari/537.36',
    /** Brave on Android sends Chrome's user agent. */
    braveAndroid:
        'Mozilla/5.0 (Linux; Android 14; Pixel 8) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/141.0.0.0 Mobile Safari/537.36',
    firefoxAndroid: 'Mozilla/5.0 (Android 14; Mobile; rv:143.0) Gecko/143.0 Firefox/143.0',
    androidWebView:
        'Mozilla/5.0 (Linux; Android 14; Pixel 7 Build/AP2A.240905.003; wv) AppleWebKit/537.36 (KHTML, like Gecko) Version/4.0 Chrome/141.0.7390.122 Mobile Safari/537.36',
    /** An older WebView: no `; wv)`, but `Version/4.0 Chrome/a.b.c.d`. */
    androidWebViewOld:
        'Mozilla/5.0 (Linux; Android 4.4.2; Nexus 5 Build/KOT49H) AppleWebKit/537.36 (KHTML, like Gecko) Version/4.0 Chrome/30.0.0.0 Mobile Safari/537.36',
    webShell:
        'Mozilla/5.0 (Linux; Android 15; Seeker Build/AP3A.241005.015; wv) AppleWebKit/537.36 (KHTML, like Gecko) Version/4.0 Chrome/141.0.7390.122 Mobile Safari/537.36 Solana Mobile Web Shell/1.2.0',
    telegramAndroid:
        'Mozilla/5.0 (Linux; Android 14; Pixel 7 Build/AP2A.240905.003; wv) AppleWebKit/537.36 (KHTML, like Gecko) Version/4.0 Chrome/141.0.7390.122 Mobile Safari/537.36 Telegram-Android/12.0.1 (Google Pixel 7; Android 14; SDK 34; HIGH)',
    facebookAndroid:
        'Mozilla/5.0 (Linux; Android 14; Pixel 7 Build/AP2A.240905.003; wv) AppleWebKit/537.36 (KHTML, like Gecko) Version/4.0 Chrome/141.0.7390.122 Mobile Safari/537.36 [FB_IAB/FB4A;FBAV/520.0.0.47.70;]',
    iphoneSafari:
        'Mozilla/5.0 (iPhone; CPU iPhone OS 18_6 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.6 Mobile/15E148 Safari/604.1',
    ipod: 'Mozilla/5.0 (iPod touch; CPU iPhone OS 15_8 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/15.6 Mobile/15E148 Safari/604.1',
    /** iPadOS (and macOS Safari) report a Macintosh; only the touch points tell them apart. */
    macSafari:
        'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.6 Safari/605.1.15',
    instagramIos:
        'Mozilla/5.0 (iPhone; CPU iPhone OS 18_6 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Mobile/22G86 Instagram 400.0.0.25.88 (iPhone15,2; iOS 18_6; en_US; en; scale=3.00; 1179x2556; 789012345)',
    facebookIos:
        'Mozilla/5.0 (iPhone; CPU iPhone OS 18_6 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Mobile/22G86 [FBAN/FBIOS;FBAV/520.0.0.38.105;FBBV/700000000;FBDV/iPhone15,2;FBMD/iPhone;FBSN/iOS;FBSV/18.6;FBSS/3;FBID/phone;FBLC/en_US;FBOP/5]',
    lineIos:
        'Mozilla/5.0 (iPhone; CPU iPhone OS 18_6 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Mobile/22G86 Safari Line/14.15.0',
    xIos: 'Mozilla/5.0 (iPhone; CPU iPhone OS 18_6 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Mobile/22G86 Twitter for iPhone/11.12',
    windowsChrome:
        'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/141.0.0.0 Safari/537.36',
    linuxChrome:
        'Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/141.0.0.0 Safari/537.36',
} as const;

export type UaName = keyof typeof UA;

interface NavigatorStub {
    readonly userAgent?: string;
    readonly maxTouchPoints?: number;
    readonly userAgentData?: { readonly mobile?: boolean } | undefined;
}

/**
 * Stubs `navigator.userAgent`, `maxTouchPoints` and `userAgentData` (own properties shadowing the
 * prototype's); the returned function restores jsdom's.
 */
export function stubNavigator(s: NavigatorStub): () => void {
    const keys = ['userAgent', 'maxTouchPoints', 'userAgentData'] as const;
    for (const k of keys) {
        if (k in s) Object.defineProperty(navigator, k, { configurable: true, get: () => s[k] });
    }
    return () => {
        for (const k of keys) {
            if (Object.hasOwn(navigator, k)) delete (navigator as unknown as Record<string, unknown>)[k];
        }
    };
}
