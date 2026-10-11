// Party text chat helpers: length cap, rate limit and a basic profanity filter.
export const CHAT_MAX = 200;

// Common profanity and slurs, matched case-insensitively as whole words,
// tolerating l33t substitutions and stretched letters ("fuuuck").
// STEMS also match common suffixes ("fucking", "shitty"); EXACT words only
// match alone so "spicy", "cocktail" and "Dickens" stay untouched.
const STEMS = [
  "fuck", "shit", "bitch", "cunt", "pussy", "asshole", "bastard", "whore", "slut",
  "faggot", "nigger", "nigga", "retard", "kike", "tranny", "twat", "wank",
];
const EXACT = ["dick", "dicks", "cock", "cocks", "fag", "fags", "spic", "spics", "chink", "chinks"];
const SUFFIX = "(?:s|es|ed|er|ers|ing|in|y|ty|ies|head|heads|face|wit|hole|holes|bag|tard)?";
const LEET: Record<string, string> = { a: "[a@4]", e: "[e3]", i: "[i1!|]", o: "[o0]", s: "[s$5]", t: "[t7]", g: "[g9]" };
const body = (w: string) => [...w].map((c) => (LEET[c] ?? c) + "+").join("[\\W_]*");
const PATTERNS = [
  ...STEMS.map((w) => new RegExp("(?<![a-z0-9])" + body(w) + SUFFIX + "(?![a-z0-9])", "gi")),
  ...EXACT.map((w) => new RegExp("(?<![a-z0-9])" + body(w) + "(?![a-z0-9])", "gi")),
];

export function cleanChat(raw: unknown): string {
  let s = String(raw ?? "")
    .replace(/[\u0000-\u001f\u007f​-‏‪-‮]/g, "")
    .replace(/\s+/g, " ")
    .trim()
    .slice(0, CHAT_MAX);
  for (const re of PATTERNS) s = s.replace(re, (m) => m[0] + "*".repeat(Math.max(2, m.length - 1)));
  return s;
}

/** Token bucket per user: 5 messages, refilling one every 1.5s. */
const buckets = new Map<string, { n: number; t: number }>();
export function chatAllowed(uid: string): boolean {
  const now = Date.now();
  const b = buckets.get(uid) ?? { n: 5, t: now };
  b.n = Math.min(5, b.n + (now - b.t) / 1500);
  b.t = now;
  buckets.set(uid, b);
  if (b.n < 1) return false;
  b.n -= 1;
  return true;
}
