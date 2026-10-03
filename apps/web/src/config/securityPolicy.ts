/**
 * The disclosure policy's values (owner decision of 2026-10-02, M10 Q3): the `/security` page
 * (`features/security/SecurityPolicyPage.tsx`), `public/.well-known/security.txt`, the repository's
 * `SECURITY.md` and the `security.txt` both programs embed (`programs/{shortwin,complete_curve}/src/
 * lib.rs`) carry the same. `securityPolicy.test.ts` holds this file to the committed security.txt.
 */

/** The security mailbox (security.txt `Contact`, the programs' `contacts` `email:` entry). */
export const SECURITY_EMAIL = 'security@short.win';

/** The policy page's canonical URL (security.txt `Policy`; the programs' `policy` and `link:`). */
export const SECURITY_POLICY_URL = 'https://short.win/security';

/** The web app's canonical security.txt (RFC 9116 `Canonical`). */
export const SECURITY_TXT_CANONICAL = 'https://short.win/.well-known/security.txt';
