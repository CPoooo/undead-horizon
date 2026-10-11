// Undead Horizon API, a Neon Function (Hono + node-postgres).
//
// Public:   GET  /health, GET /leaderboard?diff=
// Signed-in (Neon Auth JWT in `Authorization: Bearer`):
//           GET  /me                     profile (+ save summary), created on first call
//           POST /me/import              one-time import of offline progress
//           POST /me/name                change display name
//           POST /shop/buy               { cat, id }   server-checked price + ownership
//           POST /equip                  { cat, id }   must own it
//           PUT  /save                   { data, summary }
//           POST /save/consume           returns the save and deletes it (resume once)
//           POST /runs                   finished/abandoned run -> coins, progress, leaderboard
import { Hono } from "hono";
import { cors } from "hono/cors";
import { Pool } from "pg";
import { createRemoteJWKSet, jwtVerify } from "jose";
import { attachDatabasePool } from "@neon/functions";
import { PRICES, CHAPTERS, DIFF_COINS, DIFFS, CATEGORIES, waveCoins } from "./catalog";

const pool = new Pool({ connectionString: process.env.DATABASE_URL, max: 5 });
attachDatabasePool(pool);

const jwks = createRemoteJWKSet(new URL(process.env.NEON_AUTH_JWKS_URL!));
const issuer = new URL(process.env.NEON_AUTH_BASE_URL!).origin;

type Env = { Variables: { userId: string } };
const app = new Hono<Env>();

const ALLOWED_ORIGINS = [
  "https://cpoooo.github.io",
  "http://localhost:8080",
  "http://127.0.0.1:8080",
  "http://localhost:8765",
];
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

app.get("/leaderboard", async (c) => {
  const diff = c.req.query("diff");
  const filter = diff && DIFFS.includes(diff) ? diff : null;
  // Best infinite run per player, then ranked.
  const { rows } = await pool.query(
    `select * from (
       select distinct on (user_id) id, user_id, name, wave, level, kills, time, diff, gun, created_at
       from runs
       where mode = 'infinite' and not abandoned and ($1::text is null or diff = $1)
       order by user_id, wave desc, level desc, kills desc, time asc
     ) best
     order by wave desc, level desc, kills desc, time asc
     limit 25`,
    [filter],
  );
  c.header("Cache-Control", "no-store");
  return c.json(
    rows.map((r) => ({
      id: String(r.id),
      name: r.name,
      wave: r.wave,
      level: r.level,
      kills: r.kills,
      time: r.time,
      diff: r.diff,
      gun: r.gun,
      date: r.created_at,
    })),
  );
});

// ---------------------------------------------------------------- auth
app.use("/me/*", auth);
app.use("/me", auth);
app.use("/shop/*", auth);
app.use("/equip", auth);
app.use("/save", auth);
app.use("/save/*", auth);
app.use("/runs", auth);

async function auth(c: any, next: () => Promise<void>) {
  const h = c.req.header("authorization");
  if (!h?.toLowerCase().startsWith("bearer ")) return c.json({ error: "unauthorized" }, 401);
  try {
    const { payload } = await jwtVerify(h.slice(7), jwks, { issuer });
    if (!payload.sub) return c.json({ error: "unauthorized" }, 401);
    c.set("userId", payload.sub);
  } catch {
    return c.json({ error: "unauthorized" }, 401);
  }
  await next();
}

const cleanName = (s: unknown) =>
  String(s ?? "")
    .replace(/[^\p{L}\p{N} _.\-]/gu, "")
    .trim()
    .slice(0, 16);

async function ensureProfile(userId: string) {
  const existing = await pool.query("select * from profiles where user_id = $1", [userId]);
  if (existing.rows[0]) return existing.rows[0];
  const u = await pool.query(`select name, email from neon_auth."user" where id = $1`, [userId]);
  const base = u.rows[0]?.name || String(u.rows[0]?.email || "Survivor").split("@")[0];
  const name = cleanName(base) || "Survivor";
  const { rows } = await pool.query(
    `insert into profiles (user_id, display_name) values ($1, $2)
     on conflict (user_id) do update set user_id = excluded.user_id
     returning *`,
    [userId, name],
  );
  return rows[0];
}

async function profileJson(userId: string) {
  const p = await ensureProfile(userId);
  const s = await pool.query("select summary from saves where user_id = $1", [userId]);
  return {
    name: p.display_name,
    coins: p.coins,
    owned: p.owned,
    equipped: p.equipped,
    campaign: p.campaign,
    stats: p.stats,
    imported: p.imported,
    save: s.rows[0] ? { summary: s.rows[0].summary } : null,
  };
}

app.get("/me", async (c) => c.json(await profileJson(c.get("userId"))));

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
  const { rowCount } = await pool.query(
    `update profiles set equipped = jsonb_set(equipped, array[$2], to_jsonb($3::text)), updated_at = now()
     where user_id = $1 and ($2 || ':' || $3) = any(owned)`,
    [uid, cat, id],
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
    `insert into saves (user_id, data, summary) values ($1, $2, $3)
     on conflict (user_id) do update set data = excluded.data, summary = excluded.summary, created_at = now()`,
    [uid, data, sum],
  );
  return c.json({ ok: true, summary: sum });
});

app.post("/save/consume", async (c) => {
  const { rows } = await pool.query("delete from saves where user_id = $1 returning data, summary", [c.get("userId")]);
  if (!rows[0]) return c.json({ error: "no_save" }, 404);
  return c.json(rows[0]);
});

app.delete("/save", async (c) => {
  await pool.query("delete from saves where user_id = $1", [c.get("userId")]);
  return c.json({ ok: true });
});

// ---------------------------------------------------------------- runs
app.post("/runs", async (c) => {
  const uid = c.get("userId");
  const r = await c.req.json();
  const mode = r.mode === "campaign" ? "campaign" : "infinite";
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

  const p = await ensureProfile(uid);
  // Coins are capped by what the reported progress could plausibly have earned.
  const cap = waveCoins({ mode, chapter, diff, wave, victory });
  const coins = Math.min(int(r.coins, 0, 1_000_000), cap);

  const client = await pool.connect();
  let rank: number | null = null;
  try {
    await client.query("begin");
    const ins = await client.query(
      `insert into runs (user_id, name, mode, chapter, diff, gun, wave, level, kills, time, coins, victory, abandoned)
       values ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13) returning id`,
      [uid, p.display_name, mode, chapter, diff, gun, wave, level, kills, time, coins, victory, abandoned],
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
    await client.query(
      `update profiles set coins = coins + $2, campaign = $3, stats = $4, updated_at = now() where user_id = $1`,
      [uid, coins, campaign, stats],
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
    return c.json({ id: String(ins.rows[0].id), coins, rank, profile: await profileJson(uid) });
  } catch (e) {
    await client.query("rollback");
    throw e;
  } finally {
    client.release();
  }
});

export default app;
