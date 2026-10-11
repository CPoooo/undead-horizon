// Undead Horizon API, a Neon Function (Hono + node-postgres).
//
// Public:   GET  /health, GET /leaderboard?diff=, POST /errors (client crash reports),
//           POST /vendor (Home Base vendor lines; signed-in callers get a higher rate limit)
// Signed-in (Neon Auth JWT in `Authorization: Bearer`):
//           GET  /me                     profile (+ save summary), created on first call
//           POST /me/import              one-time import of offline progress
//           POST /me/name                change display name
//           POST /shop/buy               { cat, id }   server-checked price + ownership
//           POST /equip                  { cat, id }   must own it
//           PUT  /save                   { data, summary }  slot = summary.mode (infinite|campaign)
//           POST /save/consume           { slot } returns that save and deletes it (resume once)
//           DELETE /save?slot=           discard a slot (a new run of that mode overwrites it)
//           POST /runs                   finished/abandoned run -> coins, progress, leaderboard
//           GET  /turn                   WebRTC ICE servers (STUN + short-lived Metered TURN relay)
import { Hono } from "hono";
import { cors } from "hono/cors";
import { PRICES, CHAPTERS, DIFFS, CATEGORIES, waveCoins } from "./catalog";
import { pool, verifyToken, cleanName, ensureProfile } from "./shared";
import { social } from "./social";
import { vendor } from "./vendor";
// shared with the browser (window.UHProg): one source of truth for progression rules
import Prog from "../ui/progression.js";

type Env = { Variables: { userId: string } };
const app = new Hono<Env>();

const ALLOWED_ORIGINS = [
  "https://cpoooo.github.io",
  "http://localhost:8080",
  "http://127.0.0.1:8080",
  "http://localhost:8765",
];
// WebSocket route first: CORS middleware must not wrap the 101 upgrade.
app.route("/", social);

app.use(
  "*",
  cors({
    origin: (o) => (ALLOWED_ORIGINS.includes(o) ? o : null),
    allowHeaders: ["Authorization", "Content-Type"],
    allowMethods: ["GET", "POST", "PUT", "DELETE", "OPTIONS"],
    maxAge: 86400,
  }),
);

app.onError((err, c) => {
  console.error(err);
  return c.json({ error: "server_error" }, 500);
});

// ---------------------------------------------------------------- public
app.get("/health", (c) => c.json({ ok: true }));
app.route("/", vendor);

// Client crash reports. Rate-limited per address (in this isolate) and capped
// globally per hour so a crash loop can't flood the table.
const errorHits = new Map<string, { n: number; at: number }>();
app.post("/errors", async (c) => {
  const ip = c.req.header("x-forwarded-for")?.split(",")[0].trim() || "?";
  const now = Date.now();
  const h = errorHits.get(ip);
  if (h && now - h.at < 60_000 && h.n >= 5) return c.json({ ok: false, error: "rate_limited" }, 429);
  errorHits.set(ip, h && now - h.at < 60_000 ? { n: h.n + 1, at: h.at } : { n: 1, at: now });
  if (errorHits.size > 5000) errorHits.clear();
  const b = await c.req.json().catch(() => ({}));
  const cut = (v: unknown, n: number) => String(v ?? "").slice(0, n);
  const auth = c.req.header("authorization");
  const uid = auth?.toLowerCase().startsWith("bearer ") ? await verifyToken(auth.slice(7)).catch(() => null) : null;
  const { rows } = await pool.query(`select count(*)::int as n from client_errors where created_at > now() - interval '1 hour'`);
  if (rows[0].n >= 1000) return c.json({ ok: false, error: "busy" }, 429);
  await pool.query(
    `insert into client_errors (user_id, kind, message, stack, url, ua, build) values ($1,$2,$3,$4,$5,$6,$7)`,
    [uid, cut(b.kind, 20) || "error", cut(b.message, 1000), cut(b.stack, 4000), cut(b.url, 300), cut(c.req.header("user-agent"), 300), cut(b.build, 40)],
  );
  return c.json({ ok: true });
});

app.get("/leaderboard", async (c) => {
  const q = (k: string) => c.req.query(k);
  if (q("mode") === "outbreak") {
    // best round per player on one Outbreak map
    const map = String(q("map") || "asylum").slice(0, 20);
    const { rows } = await pool.query(
      `select * from (
         select distinct on (user_id) id, user_id, name, wave, level, kills, time, diff, gun, players, score, created_at
         from runs where mode = 'outbreak' and map = $1 and not abandoned
         order by user_id, wave desc, score desc
       ) best order by wave desc, score desc limit 25`,
      [map],
    );
    c.header("Cache-Control", "no-store");
    return c.json(rows.map(lbRow));
  }
  if (q("daily")) {
    // today's daily run: each player's FIRST attempt counts
    const day = String(q("daily")).slice(0, 10);
    const { rows } = await pool.query(
      `select * from (
         select distinct on (user_id) id, user_id, name, wave, level, kills, time, diff, gun, players, score, created_at
         from runs where daily = $1 order by user_id, created_at asc
       ) first order by wave desc, score desc, kills desc limit 25`,
      [day],
    );
    c.header("Cache-Control", "no-store");
    return c.json(rows.map(lbRow));
  }
  // filters: diff, scope=week, players=solo|coop, pure=1, friends=1 (signed in)
  const where = ["mode = 'infinite'", "not abandoned", "not mutated", "daily = ''"];
  const args: unknown[] = [];
  const diff = q("diff");
  if (diff && DIFFS.includes(diff)) {
    args.push(diff);
    where.push(`diff = $${args.length}`);
  }
  if (q("scope") === "week") where.push("created_at > now() - interval '7 days'");
  if (q("players") === "solo") where.push("players = 1");
  if (q("players") === "coop") where.push("players > 1");
  if (q("pure") === "1") where.push("pure");
  if (q("friends") === "1") {
    const h = c.req.header("authorization");
    const uid = h?.toLowerCase().startsWith("bearer ") ? await verifyToken(h.slice(7)).catch(() => null) : null;
    if (!uid) return c.json([]);
    args.push(uid);
    where.push(`(user_id = $${args.length} or user_id in (select friend_id from friends where user_id = $${args.length} and status = 'accepted'))`);
  }
  // Best run per player, then ranked.
  const { rows } = await pool.query(
    `select * from (
       select distinct on (user_id) id, user_id, name, wave, level, kills, time, diff, gun, players, score, created_at
       from runs
       where ${where.join(" and ")}
       order by user_id, wave desc, level desc, kills desc, time asc
     ) best
     order by wave desc, level desc, kills desc, time asc
     limit 25`,
    args,
  );
  c.header("Cache-Control", "no-store");
  return c.json(rows.map(lbRow));
});

function lbRow(r: any) {
  return {
    id: String(r.id),
    name: r.name,
    wave: r.wave,
    level: r.level,
    kills: r.kills,
    time: r.time,
    diff: r.diff,
    gun: r.gun,
    players: r.players,
    score: Number(r.score ?? 0),
    date: r.created_at,
  };
}

// ---------------------------------------------------------------- auth
app.use("/me/*", auth);
app.use("/me", auth);
app.use("/shop/*", auth);
app.use("/equip", auth);
app.use("/save", auth);
app.use("/save/*", auth);
app.use("/runs", auth);
app.use("/turn", auth);

async function auth(c: any, next: () => Promise<void>) {
  const h = c.req.header("authorization");
  const uid = h?.toLowerCase().startsWith("bearer ") ? await verifyToken(h.slice(7)) : null;
  if (!uid) return c.json({ error: "unauthorized" }, 401);
  c.set("userId", uid);
  await next();
}

async function profileJson(userId: string) {
  const p = await ensureProfile(userId);
  const s = await pool.query("select slot, summary from saves where user_id = $1", [userId]);
  const saves: Record<string, unknown> = { infinite: null, campaign: null };
  for (const r of s.rows) saves[r.slot] = { summary: r.summary };
  return {
    name: p.display_name,
    coins: p.coins,
    owned: p.owned,
    equipped: p.equipped,
    campaign: p.campaign,
    stats: p.stats,
    imported: p.imported,
    friendCode: p.friend_code,
    saves,
    progress: Prog.normalize(p.progress),
    settings: p.settings ?? {},
  };
}

app.get("/me", async (c) => c.json(await profileJson(c.get("userId"))));

// settings follow the account across devices (small, flat JSON)
app.put("/me/settings", async (c) => {
  const uid = c.get("userId");
  const body = await c.req.json().catch(() => ({}));
  const clean: Record<string, unknown> = {};
  for (const [k, v] of Object.entries(body ?? {}).slice(0, 80)) {
    if (!/^[a-zA-Z]{1,24}$/.test(k)) continue;
    if (typeof v === "number" || typeof v === "boolean") clean[k] = v;
    else if (typeof v === "string") clean[k] = v.slice(0, 400);
    else if (Array.isArray(v)) clean[k] = v.slice(0, 20).map((x) => String(x).slice(0, 40));
  }
  await pool.query("update profiles set settings = $2 where user_id = $1", [uid, clean]);
  return c.json({ ok: true });
});

// ---------------------------------------------------------------- TURN relay
// Players whose networks block direct WebRTC (cellular CGNAT, Wi-Fi client
// isolation) fall back to Metered's relay. The secret key never leaves the
// server: we mint one shared credential per isolate and refresh it hourly.
const STUN_ONLY = [{ urls: ["stun:stun.l.google.com:19302", "stun:stun1.l.google.com:19302"] }];
const meteredHost = () => {
  const a = (process.env.METERED_APP ?? "").replace(/^https?:\/\//, "").split("/")[0].trim();
  return !a ? "" : a.endsWith(".metered.live") ? a : `${a}.metered.live`;
};
let turnCache: { at: number; servers: unknown[] } | null = null;
async function turnServers(): Promise<unknown[]> {
  if (turnCache && Date.now() - turnCache.at < 55 * 60_000) return turnCache.servers;
  const host = meteredHost();
  if (!host) return STUN_ONLY;
  let apiKey = process.env.METERED_API_KEY;
  if (!apiKey && process.env.METERED_SECRET_KEY) {
    const r = await fetch(`https://${host}/api/v1/turn/credential?secretKey=${encodeURIComponent(process.env.METERED_SECRET_KEY)}`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ expiryInSeconds: 4 * 3600, label: "undead-horizon" }),
    });
    if (!r.ok) throw new Error(`metered credential ${r.status}`);
    apiKey = (await r.json()).apiKey;
  }
  if (!apiKey) return STUN_ONLY;
  const r = await fetch(`https://${host}/api/v1/turn/credentials?apiKey=${encodeURIComponent(apiKey)}`);
  if (!r.ok) throw new Error(`metered ice ${r.status}`);
  const servers = await r.json();
  if (!Array.isArray(servers) || !servers.length) return STUN_ONLY;
  turnCache = { at: Date.now(), servers: [...STUN_ONLY, ...servers] };
  return turnCache.servers;
}

app.get("/turn", async (c) => {
  try {
    return c.json({ iceServers: await turnServers() });
  } catch (e) {
    console.error("[turn]", e);
    return c.json({ iceServers: STUN_ONLY });
  }
});

app.post("/me/name", async (c) => {
  const { name } = await c.req.json();
  const n = cleanName(name);
  if (n.length < 2) return c.json({ error: "name_too_short" }, 400);
  await ensureProfile(c.get("userId"));
  await pool.query("update profiles set display_name = $2, updated_at = now() where user_id = $1", [c.get("userId"), n]);
  return c.json(await profileJson(c.get("userId")));
});

// One-time carry-over of progress earned while playing signed-out.
app.post("/me/import", async (c) => {
  const uid = c.get("userId");
  const p = await ensureProfile(uid);
  if (p.imported) return c.json(await profileJson(uid));
  const body = await c.req.json().catch(() => ({}));
  const coins = Math.max(0, Math.min(5000, Math.floor(Number(body.coins) || 0)));
  const owned: string[] = Array.isArray(body.owned)
    ? body.owned.filter((k: unknown) => typeof k === "string" && k in PRICES)
    : [];
  const unlocked = Math.max(0, Math.min(CHAPTERS.length - 1, Math.floor(Number(body.campaign?.unlocked) || 0)));
  const cleared = Array.isArray(body.campaign?.cleared)
    ? body.campaign.cleared.filter((n: unknown) => Number.isInteger(n) && (n as number) >= 0 && (n as number) < CHAPTERS.length)
    : [];
  await pool.query(
    `update profiles set
       coins = coins + $2,
       owned = (select array(select distinct unnest(owned || $3::text[]))),
       campaign = jsonb_build_object(
         'unlocked', greatest((campaign->>'unlocked')::int, $4),
         'cleared', (select coalesce(jsonb_agg(distinct x), '[]'::jsonb)
                     from jsonb_array_elements(campaign->'cleared' || $5::jsonb) x)),
       imported = true, updated_at = now()
     where user_id = $1`,
    [uid, coins, owned, unlocked, JSON.stringify(cleared)],
  );
  return c.json(await profileJson(uid));
});

// ---------------------------------------------------------------- shop & locker
app.post("/shop/buy", async (c) => {
  const uid = c.get("userId");
  const { cat, id } = await c.req.json();
  const key = `${cat}:${id}`;
  const price = PRICES[key];
  if (!CATEGORIES.includes(cat) || price === undefined) return c.json({ error: "unknown_item" }, 400);
  await ensureProfile(uid);
  // Atomic: only succeeds if affordable and not already owned.
  const { rowCount } = await pool.query(
    `update profiles set coins = coins - $3, owned = array_append(owned, $2),
       equipped = jsonb_set(equipped, array[$4], to_jsonb($5::text)), updated_at = now()
     where user_id = $1 and coins >= $3 and not ($2 = any(owned))`,
    [uid, key, price, cat, id],
  );
  if (!rowCount) return c.json({ error: "cannot_buy" }, 409);
  return c.json(await profileJson(uid));
});

app.post("/equip", async (c) => {
  const uid = c.get("userId");
  const { cat, id } = await c.req.json();
  if (!CATEGORIES.includes(cat)) return c.json({ error: "bad_category" }, 400);
  await ensureProfile(uid);
  // free items (survivors) need no ownership row
  // free defaults (survivors, the starter cosmetics) need no ownership row
  const free = PRICES[`${cat}:${id}`] === 0;
  const { rowCount } = await pool.query(
    `update profiles set equipped = jsonb_set(equipped, array[$2], to_jsonb($3::text)), updated_at = now()
     where user_id = $1 and (($2 || ':' || $3) = any(owned) or $4)`,
    [uid, cat, id, free],
  );
  if (!rowCount) return c.json({ error: "not_owned" }, 409);
  return c.json(await profileJson(uid));
});

// ---------------------------------------------------------------- saves
app.put("/save", async (c) => {
  const uid = c.get("userId");
  const { data, summary } = await c.req.json();
  if (typeof data !== "string" || data.length > 4096) return c.json({ error: "bad_save" }, 400);
  await ensureProfile(uid);
  const sum = {
    mode: summary?.mode === "campaign" ? "campaign" : "infinite",
    chapter: Math.max(0, Math.min(CHAPTERS.length - 1, Number(summary?.chapter) || 0)),
    diff: DIFFS.includes(summary?.diff) ? summary.diff : "survivor",
    wave: Math.max(1, Number(summary?.wave) || 1),
    level: Math.max(1, Number(summary?.level) || 1),
    date: Date.now(),
  };
  await pool.query(
    `insert into saves (user_id, slot, data, summary) values ($1, $2, $3, $4)
     on conflict (user_id, slot) do update set data = excluded.data, summary = excluded.summary, created_at = now()`,
    [uid, sum.mode, data, sum],
  );
  return c.json({ ok: true, summary: sum });
});

const slotOf = (v: unknown) => (v === "campaign" ? "campaign" : "infinite");

app.post("/save/consume", async (c) => {
  const body = await c.req.json().catch(() => ({}));
  const { rows } = await pool.query(
    "delete from saves where user_id = $1 and slot = $2 returning data, summary",
    [c.get("userId"), slotOf(body.slot)],
  );
  if (!rows[0]) return c.json({ error: "no_save" }, 404);
  return c.json(rows[0]);
});

app.delete("/save", async (c) => {
  await pool.query("delete from saves where user_id = $1 and slot = $2", [c.get("userId"), slotOf(c.req.query("slot"))]);
  return c.json({ ok: true });
});

// ---------------------------------------------------------------- runs
app.post("/runs", async (c) => {
  const uid = c.get("userId");
  const r = await c.req.json();
  const mode = ["campaign", "outbreak", "siege"].includes(r.mode) ? r.mode : "infinite";
  const diff = DIFFS.includes(r.diff) ? r.diff : "survivor";
  const chapter = Math.max(0, Math.min(CHAPTERS.length - 1, Math.floor(Number(r.chapter) || 0)));
  const int = (v: unknown, lo: number, hi: number) => Math.max(lo, Math.min(hi, Math.floor(Number(v) || 0)));
  const wave = int(r.wave, 1, 10000);
  const level = int(r.level, 1, 10000);
  const kills = int(r.kills, 0, 10_000_000);
  const time = Math.max(0, Number(r.time) || 0);
  const victory = mode === "campaign" && !!r.victory && wave >= CHAPTERS[chapter].waves;
  const abandoned = !!r.abandoned;
  const gun = String(r.gun || "pistol").slice(0, 20);
  const players = Math.max(1, Math.min(3, Math.floor(Number(r.players) || 1)));

  const p = await ensureProfile(uid);
  // Coins are capped by what the reported progress could plausibly have earned.
  const cap = waveCoins({ mode, chapter, diff, wave, victory, mutated: !!r.mutated });
  const coins = Math.min(int(r.coins, 0, 1_000_000), cap);

  const client = await pool.connect();
  let rank: number | null = null;
  try {
    await client.query("begin");
    const score = int(r.score, 0, 2_000_000_000);
    const map = String(r.map || "").replace(/[^a-z0-9_]/g, "").slice(0, 20);
    const today = new Date().toISOString().slice(0, 10);
    const daily = r.daily === today ? today : "";
    const mutated = !!r.mutated;
    const pure = !!r.pure;
    const cause = String(r.cause || "").slice(0, 40);
    const build = String(r.build || "").replace(/[^0-9:,]/g, "").slice(0, 200);
    const ins = await client.query(
      `insert into runs (user_id, name, mode, chapter, diff, gun, wave, level, kills, time, coins, victory, abandoned, players, score, map, daily, mutated, pure, cause, build)
       values ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16,$17,$18,$19,$20,$21) returning id`,
      [uid, p.display_name, mode, chapter, diff, gun, wave, level, kills, time, coins, victory, abandoned, players, score, map, daily, mutated, pure, cause, build],
    );
    const campaign = p.campaign as { unlocked: number; cleared: number[] };
    if (victory) {
      if (!campaign.cleared.includes(chapter)) campaign.cleared.push(chapter);
      campaign.unlocked = Math.max(campaign.unlocked, Math.min(chapter + 1, CHAPTERS.length - 1));
    }
    const stats = p.stats as { runs: number; kills: number; bestWave: number };
    stats.runs += 1;
    stats.kills += kills;
    if (mode === "infinite") stats.bestWave = Math.max(stats.bestWave || 0, wave);
    // career: xp, levels (coins + items), achievements, mastery, bestiary, relic drops
    const kk = Array.isArray(r.kk) ? r.kk.slice(0, 15).map((n: unknown) => int(n, 0, 100000)) : [];
    const bosses = int(r.bosses, 0, Math.ceil(wave / 5) + 4);
    const career = Prog.applyRun(p.progress, {
      mode, diff, wave, level, kills, time, victory, players, gun, score, kk, bosses,
      combo: int(r.combo, 0, 100000), evos: int(r.evos, 0, 8), chests: int(r.chests, 0, 200),
      stars: int(r.stars, 0, 3), pure: !!r.pure, chapter, daily: r.daily === new Date().toISOString().slice(0, 10) ? r.daily : "",
    });
    const owned = new Set(p.owned as string[]);
    for (const it of career.items) owned.add(it);
    await client.query(
      `update profiles set coins = coins + $2, campaign = $3, stats = $4, progress = $5, owned = $6, updated_at = now() where user_id = $1`,
      [uid, coins + career.coins, campaign, stats, career.progress, [...owned]],
    );
    await client.query("commit");
    if (mode === "infinite" && !abandoned) {
      // rank of this run among each player's best runs
      const rr = await pool.query(
        `select count(*)::int + 1 as rank from (
           select distinct on (user_id) user_id, wave, level, kills, time from runs
           where mode = 'infinite' and not abandoned and user_id <> $1
           order by user_id, wave desc, level desc, kills desc, time asc) b
         where (b.wave, b.level, b.kills, -b.time) > ($2, $3, $4, -$5::real)`,
        [uid, wave, level, kills, time],
      );
      rank = rr.rows[0].rank;
    }
    return c.json({
      id: String(ins.rows[0].id),
      coins,
      rank,
      career: { xp: career.xp, levelsGained: career.levelsGained, coins: career.coins, items: career.items, achievements: career.achievements, relics: career.relics },
      profile: await profileJson(uid),
    });
  } catch (e) {
    await client.query("rollback");
    throw e;
  } finally {
    client.release();
  }
});

// ---------------------------------------------------------------- career
app.use("/meta/*", auth);
app.use("/relics/*", auth);
app.use("/achievement", auth);

/** Welcome back: a gentle bonus after 3+ days away (no streaks to lose). */
app.use("/welcome", auth);
app.post("/welcome", async (c) => {
  const uid = c.get("userId");
  const p = await ensureProfile(uid);
  const away = Date.now() - new Date(p.updated_at ?? Date.now()).getTime();
  const last = p.last_bonus ? Date.now() - new Date(p.last_bonus).getTime() : Infinity;
  if (away < 3 * 86_400_000 || last < 3 * 86_400_000) return c.json({ bonus: 0, profile: await profileJson(uid) });
  const bonus = 150;
  await pool.query("update profiles set coins = coins + $2, last_bonus = now(), updated_at = now() where user_id = $1", [uid, bonus]);
  return c.json({ bonus, profile: await profileJson(uid) });
});

/** Buy one rank of a permanent upgrade. */
app.post("/meta/buy", async (c) => {
  const uid = c.get("userId");
  const { id } = await c.req.json();
  const def = Prog.META.find((m: { id: string }) => m.id === id);
  if (!def) return c.json({ error: "bad_upgrade" }, 400);
  const p = await ensureProfile(uid);
  const prog = Prog.normalize(p.progress);
  const rank = prog.meta[id] || 0;
  if (rank >= def.max) return c.json({ error: "maxed" }, 409);
  const cost = Prog.metaCost(id, rank);
  prog.meta[id] = rank + 1;
  const { rowCount } = await pool.query(
    "update profiles set coins = coins - $2, progress = $3, updated_at = now() where user_id = $1 and coins >= $2",
    [uid, cost, prog],
  );
  if (!rowCount) return c.json({ error: "insufficient_coins" }, 409);
  return c.json(await profileJson(uid));
});

/** Equip up to three relics you own. */
app.post("/relics/equip", async (c) => {
  const uid = c.get("userId");
  const { uids } = await c.req.json();
  const p = await ensureProfile(uid);
  const prog = Prog.normalize(p.progress);
  prog.equippedRelics = (Array.isArray(uids) ? uids : [])
    .map(String)
    .filter((u: string) => prog.relics.some((r: { uid: string }) => r.uid === u))
    .slice(0, 3);
  await pool.query("update profiles set progress = $2, updated_at = now() where user_id = $1", [uid, prog]);
  return c.json(await profileJson(uid));
});

/** Event achievements the game reports directly (secrets). */
app.post("/achievement", async (c) => {
  const uid = c.get("userId");
  const { id } = await c.req.json();
  if (!Prog.EVENT_ACHIEVEMENTS.includes(id)) return c.json({ error: "bad_achievement" }, 400);
  const p = await ensureProfile(uid);
  const prog = Prog.normalize(p.progress);
  if (!prog.achievements.includes(id)) prog.achievements.push(id);
  await pool.query("update profiles set progress = $2, updated_at = now() where user_id = $1", [uid, prog]);
  return c.json(await profileJson(uid));
});

export default app;
