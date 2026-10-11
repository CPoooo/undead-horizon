// Cloud backend: Neon Managed Auth (Better Auth) + the `api` Neon Function.
// Bundled to ui/cloud.js with `npm run build:cloud` (esbuild) so the game has no
// runtime CDN dependency. Registers window.UHCloud, which ui/app.js uses when
// window.UH_CONFIG.apiBase/authUrl are set.
import { createAuthClient } from '@neondatabase/auth';

const cfg = window.UH_CONFIG || {};
if (cfg.apiBase && cfg.authUrl) {
  const API = cfg.apiBase.replace(/\/$/, '');
  const auth = createAuthClient(cfg.authUrl);

  let token = null;
  let tokenExp = 0;
  async function getToken(force) {
    if (!force && token && Date.now() < tokenExp - 60_000) return token;
    let jwt = null;
    try {
      const { data } = await auth.token();
      jwt = data?.token || null;
    } catch {}
    if (!jwt) {
      // After a page reload the cached session can lack the JWT; ask the JWT
      // endpoint directly (the session cookie still identifies the user).
      const r = await fetch(cfg.authUrl.replace(/\/$/, '') + '/token', { credentials: 'include' }).catch(() => null);
      if (r && r.ok) jwt = (await r.json().catch(() => ({}))).token || null;
    }
    if (!jwt) throw new Error('not_signed_in');
    token = jwt;
    try {
      tokenExp = JSON.parse(atob(token.split('.')[1].replace(/-/g, '+').replace(/_/g, '/'))).exp * 1000;
    } catch {
      tokenExp = Date.now() + 5 * 60_000;
    }
    return token;
  }

  async function api(path, { method = 'GET', body, authed = true } = {}) {
    const go = async (force) => {
      const headers = { 'Content-Type': 'application/json' };
      if (authed) headers.Authorization = 'Bearer ' + (await getToken(force));
      return fetch(API + path, { method, headers, body: body === undefined ? undefined : JSON.stringify(body) });
    };
    let res = await go(false);
    if (res.status === 401 && authed) res = await go(true); // token may have just expired
    const json = await res.json().catch(() => ({}));
    if (!res.ok) {
      const err = new Error(json.error || `http_${res.status}`);
      err.status = res.status;
      throw err;
    }
    return json;
  }

  async function authPost(path, body) {
    const r = await fetch(cfg.authUrl.replace(/\/$/, '') + path, {
      method: 'POST',
      credentials: 'include',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(body),
    });
    if (!r.ok) {
      const j = await r.json().catch(() => ({}));
      throw new Error(errText({ message: j.message || j.code || `HTTP ${r.status}` }));
    }
    return r.json().catch(() => ({}));
  }

  const errText = (error) => {
    if (!error) return 'Something went wrong';
    const m = error.message || error.code || String(error);
    if (/invalid.*(email|password)|credentials/i.test(m)) return 'Wrong email or password.';
    if (/already exists|USER_ALREADY_EXISTS/i.test(m)) return 'An account with that email already exists.';
    if (/password.*(short|least)/i.test(m)) return 'Password must be at least 8 characters.';
    if (/otp|code/i.test(m) && /invalid|expired|incorrect/i.test(m)) return 'That code is invalid or expired.';
    if (/too many|rate/i.test(m)) return 'Too many attempts. Wait a minute and try again.';
    return m;
  };

  window.UHCloud = {
    name: 'cloud',
    user: null,
    api,
    /** Fresh Neon Auth JWT (used for the social WebSocket). */
    token: () => getToken(),
    wsUrl: API.replace(/^http/, 'ws') + '/ws',
    /** Leaderboard is public: works signed in or out. */
    leaderboard: (f) => api('/leaderboard' + (f && f.startsWith('ob:') ? '?mode=outbreak&map=' + encodeURIComponent(f.slice(3))
      : f && f !== 'all' ? '?diff=' + encodeURIComponent(f) : ''), { authed: false }),
    async restore() {
      try {
        const { data } = await auth.getSession();
        this.user = data?.user || null;
      } catch {
        this.user = null;
      }
      return !!this.user;
    },
    async signIn(email, password) {
      const { data, error } = await auth.signIn.email({ email, password });
      if (error) throw new Error(errText(error));
      this.user = data?.user || (await this.restore(), this.user);
      return this.user;
    },
    async signUp(name, email, password) {
      const { data, error } = await auth.signUp.email({ name, email, password });
      if (error) throw new Error(errText(error));
      this.user = data?.user || (await this.restore(), this.user);
      if (!this.user) throw new Error('Check your email to verify your account, then sign in.');
      return this.user;
    },
    async google() {
      // All three URLs must point back at the game. Better Auth otherwise sends
      // first-time users to the bare origin, which 404s on GitHub Pages.
      const back = location.origin + location.pathname;
      const { error } = await auth.signIn.social({
        provider: 'google',
        callbackURL: back,
        newUserCallbackURL: back,
        errorCallbackURL: back,
      });
      if (error) throw new Error(errText(error));
    },
    /** Email a 6-digit password-reset code (sent via the branch's email provider). */
    async requestReset(email) {
      await authPost('/email-otp/request-password-reset', { email });
    },
    async resetPassword(email, otp, password) {
      await authPost('/email-otp/reset-password', { email, otp, password });
    },
    async signOut() {
      await auth.signOut().catch(() => {});
      token = null;
      tokenExp = 0;
      this.user = null;
    },
  };
}
