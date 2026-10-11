// Social + co-op lobby over WebSockets: friends, presence, parties (max 4),
// invites, ready checks, and WebRTC signaling relay for peer-to-peer gameplay.
//
// Neon may run several isolates, each with its own sockets, so nothing is sent
// directly between users. Every notification is a row in `events`; each
// isolate polls that table and delivers rows addressed to users connected to
// it. Postgres stays the single source of truth for lobby state.
//
// Client -> server: { t: "friends.add", code } | { t: "friends.accept"|"friends.remove", id }
//   | { t: "party.create"|"party.leave"|"party.start"|"party.end" }
//   | { t: "party.invite"|"party.kick", id } | { t: "party.join"|"party.decline", partyId }
//   | { t: "party.ready", ready } | { t: "party.diff", diff } | { t: "party.mode", mode, chapter? } | { t: "party.look", look }
//   | { t: "signal", to, data } | { t: "ping" }
//   | { t: "chat", text } | { t: "party.voice", on } | { t: "block"|"unblock", id } | { t: "report", id, reason, message }
// Server -> client: hello, social, party, party.start, invite, signal, chat, toast, error, ping
import { Hono } from "hono";
import { upgradeWebSocket } from "@neon/functions/hono";
import { pool, verifyToken, ensureProfile } from "./shared";
import { DIFFS } from "./catalog";
import { cleanChat, chatAllowed } from "./chat";

const MAX_PARTY = 4;
const ONLINE_WINDOW = "45 seconds";

type WS = WebSocket;
const sockets = new Map<string, Set<WS>>(); // userId -> this isolate's sockets

const sendWs = (ws: WS, msg: unknown) => {
  if (ws.readyState === ws.OPEN) ws.send(JSON.stringify(msg));
};
const sendUser = (uid: string, msg: unknown) => {
  for (const ws of sockets.get(uid) ?? []) sendWs(ws, msg);
};

// ---------------------------------------------------------------- fan-out
async function notify(userIds: string[], payload: Record<string, unknown>) {
  const ids = [...new Set(userIds)].filter(Boolean);
  if (!ids.length) return;
  await pool.query(
    `insert into events (user_id, payload) select unnest($1::text[]), $2::jsonb`,
    [ids, JSON.stringify(payload)],
  );
}

let cursor = -1n;
let floor = 0n; // never replay events from before this isolate started polling
let polling = false;
const delivered = new Set<string>(); // recent ids: re-scan a window so late commits aren't skipped
let lastCleanup = 0;

async function poll() {
  if (polling || sockets.size === 0) return;
  polling = true;
  try {
    if (cursor < 0n) {
      const r = await pool.query("select coalesce(max(id), 0)::text as id from events");
      cursor = BigInt(r.rows[0].id);
      floor = cursor;
    }
    const back = cursor > 200n ? cursor - 200n : 0n;
    const from = back > floor ? back : floor;
    const { rows } = await pool.query(
      "select id::text as id, user_id, payload from events where id > $1 order by id limit 2000",
      [from.toString()],
    );
    for (const r of rows) {
      const id = BigInt(r.id);
      if (id > cursor) cursor = id;
      if (delivered.has(r.id)) continue;
      delivered.add(r.id);
      if (sockets.has(r.user_id)) await deliver(r.user_id, r.payload).catch((e) => console.error("[deliver]", e));
    }
    if (delivered.size > 5000) {
      const keep = [...delivered].slice(-1000);
      delivered.clear();
      keep.forEach((k) => delivered.add(k));
    }
    if (Date.now() - lastCleanup > 60_000) {
      lastCleanup = Date.now();
      await pool.query("delete from events where created_at < now() - interval '5 minutes'");
      // parties nobody has been online in for a while
      await pool.query(
        `delete from parties p where p.created_at < now() - interval '15 minutes' and not exists (
           select 1 from party_members m join presence pr on pr.user_id = m.user_id
           where m.party_id = p.id and pr.last_seen > now() - interval '10 minutes')`,
      );
    }
  } catch (e) {
    console.error("[poll]", e);
  } finally {
    polling = false;
  }
}
setInterval(poll, 250).unref?.();
setInterval(() => {
  for (const set of sockets.values()) for (const ws of set) sendWs(ws, { t: "ping" });
}, 25_000).unref?.();

async function deliver(uid: string, p: any) {
  switch (p.k) {
    case "social":
      return sendUser(uid, { t: "social", ...(await socialState(uid)) });
    case "party":
      return sendUser(uid, { t: "party", party: await partyState(uid) });
    case "start":
      return sendUser(uid, { t: "party.start", party: await partyState(uid) });
    case "invite":
      sendUser(uid, { t: "invite", partyId: p.partyId, fromName: p.fromName });
      return sendUser(uid, { t: "social", ...(await socialState(uid)) });
    case "signal":
      return sendUser(uid, { t: "signal", from: p.from, data: p.data });
    case "toast":
      return sendUser(uid, { t: "toast", msg: p.msg });
    case "chat":
      return sendUser(uid, { t: "chat", from: p.from, name: p.name, text: p.text, ts: p.ts });
    case "hub":
      return sendUser(uid, { t: "hub", from: p.from, name: p.name, x: p.x, y: p.y, aim: p.aim, look: p.look });
  }
}

// ---------------------------------------------------------------- state
async function socialState(uid: string) {
  const friends = await pool.query(
    `select p.user_id as id, p.display_name as name,
            coalesce(pr.last_seen > now() - interval '${ONLINE_WINDOW}', false) as online,
            pm.party_id::text as "partyId"
     from friends f
     join profiles p on p.user_id = f.friend_id
     left join presence pr on pr.user_id = p.user_id
     left join party_members pm on pm.user_id = p.user_id
     where f.user_id = $1 and f.status = 'accepted'
     order by online desc, p.display_name`,
    [uid],
  );
  const incoming = await pool.query(
    `select p.user_id as id, p.display_name as name from friends f join profiles p on p.user_id = f.user_id
     where f.friend_id = $1 and f.status = 'pending' order by f.created_at desc`,
    [uid],
  );
  const outgoing = await pool.query(
    `select p.user_id as id, p.display_name as name from friends f join profiles p on p.user_id = f.friend_id
     where f.user_id = $1 and f.status = 'pending' order by f.created_at desc`,
    [uid],
  );
  const invites = await pool.query(
    `select i.party_id::text as "partyId", p.display_name as "fromName", i.from_id as "fromId"
     from party_invites i join profiles p on p.user_id = i.from_id
     join parties pa on pa.id = i.party_id
     where i.to_id = $1 and pa.status = 'open' order by i.created_at desc`,
    [uid],
  );
  const blocked = await pool.query(
    `select p.user_id as id, p.display_name as name from blocks b join profiles p on p.user_id = b.blocked_id
     where b.user_id = $1 order by b.created_at desc`,
    [uid],
  );
  return { friends: friends.rows, incoming: incoming.rows, outgoing: outgoing.rows, invites: invites.rows, blocked: blocked.rows };
}

async function partyOf(uid: string) {
  const r = await pool.query(
    `select pa.* from party_members m join parties pa on pa.id = m.party_id where m.user_id = $1`,
    [uid],
  );
  return r.rows[0] ?? null;
}

async function memberIds(partyId: string | number) {
  const r = await pool.query("select user_id from party_members where party_id = $1", [partyId]);
  return r.rows.map((x) => x.user_id as string);
}

async function partyState(uid: string) {
  const pa = await partyOf(uid);
  if (!pa) return null;
  const m = await pool.query(
    `select m.user_id as id, p.display_name as name, m.ready, m.look, m.voice,
            coalesce(pr.last_seen > now() - interval '${ONLINE_WINDOW}', false) as online
     from party_members m join profiles p on p.user_id = m.user_id
     left join presence pr on pr.user_id = m.user_id
     where m.party_id = $1 order by m.joined_at`,
    [pa.id],
  );
  return {
    id: String(pa.id),
    token: pa.token, // only ever sent to members: it lets anyone with the link join
    hostId: pa.host_id,
    diff: pa.diff,
    mode: pa.mode ?? "infinite",
    chapter: pa.chapter ?? 0,
    status: pa.status,
    seed: pa.seed ? String(pa.seed) : null,
    members: m.rows,
  };
}

async function friendIds(uid: string) {
  const r = await pool.query("select friend_id from friends where user_id = $1 and status = 'accepted'", [uid]);
  return r.rows.map((x) => x.friend_id as string);
}

async function touchPresence(uid: string, online: boolean) {
  await pool.query(
    `insert into presence (user_id, last_seen) values ($1, ${online ? "now()" : "now() - interval '10 minutes'"})
     on conflict (user_id) do update set last_seen = excluded.last_seen`,
    [uid],
  );
}

async function leaveParty(uid: string) {
  const pa = await partyOf(uid);
  if (!pa) return;
  await pool.query("delete from party_members where party_id = $1 and user_id = $2", [pa.id, uid]);
  const rest = await pool.query(
    "select user_id from party_members where party_id = $1 order by joined_at",
    [pa.id],
  );
  if (!rest.rows.length) {
    await pool.query("delete from parties where id = $1", [pa.id]);
  } else if (pa.host_id === uid) {
    const newHost = rest.rows[0].user_id;
    await pool.query("update parties set host_id = $2, status = 'open' where id = $1", [pa.id, newHost]);
    await pool.query("update party_members set ready = true where party_id = $1 and user_id = $2", [pa.id, newHost]);
  }
  await notify(rest.rows.map((r) => r.user_id), { k: "party" });
  await notify([uid], { k: "party" });
}

// ---------------------------------------------------------------- commands
async function handle(uid: string, msg: any): Promise<void> {
  switch (msg?.t) {
    case "ping":
      return touchPresence(uid, true);

    case "friends.add": {
      const code = String(msg.code ?? "").trim().toUpperCase();
      const t = await pool.query("select user_id, display_name from profiles where friend_code = $1", [code]);
      const other = t.rows[0];
      if (!other) throw new Error("No player has that friend code.");
      if (other.user_id === uid) throw new Error("That's your own code.");
      const back = await pool.query(
        "select 1 from friends where user_id = $1 and friend_id = $2 and status = 'pending'",
        [other.user_id, uid],
      );
      if (back.rowCount) return handle(uid, { t: "friends.accept", id: other.user_id });
      await pool.query(
        "insert into friends (user_id, friend_id, status) values ($1, $2, 'pending') on conflict do nothing",
        [uid, other.user_id],
      );
      const me = await ensureProfile(uid);
      await notify([uid, other.user_id], { k: "social" });
      await notify([other.user_id], { k: "toast", msg: `${me.display_name} sent you a friend request` });
      return;
    }
    case "friends.accept": {
      const id = String(msg.id ?? "");
      const r = await pool.query(
        "update friends set status = 'accepted' where user_id = $1 and friend_id = $2 and status = 'pending'",
        [id, uid],
      );
      if (!r.rowCount) throw new Error("No such friend request.");
      await pool.query(
        `insert into friends (user_id, friend_id, status) values ($1, $2, 'accepted')
         on conflict (user_id, friend_id) do update set status = 'accepted'`,
        [uid, id],
      );
      await notify([uid, id], { k: "social" });
      return;
    }
    case "friends.remove": {
      const id = String(msg.id ?? "");
      await pool.query(
        "delete from friends where (user_id = $1 and friend_id = $2) or (user_id = $2 and friend_id = $1)",
        [uid, id],
      );
      await notify([uid, id], { k: "social" });
      return;
    }

    case "party.create": {
      if (await partyOf(uid)) return sendUser(uid, { t: "party", party: await partyState(uid) });
      const pa = await pool.query("insert into parties (host_id) values ($1) returning id", [uid]);
      await pool.query(
        "insert into party_members (party_id, user_id, ready, look) values ($1, $2, true, $3)",
        [pa.rows[0].id, uid, msg.look ?? {}],
      );
      await notify([uid], { k: "party" });
      await notify(await friendIds(uid), { k: "social" });
      return;
    }
    case "party.invite": {
      const pa = await partyOf(uid);
      if (!pa) throw new Error("Create a party first.");
      if (pa.status !== "open") throw new Error("The game has already started.");
      const id = String(msg.id ?? "");
      const fr = await pool.query(
        "select 1 from friends where user_id = $1 and friend_id = $2 and status = 'accepted'",
        [uid, id],
      );
      if (!fr.rowCount) throw new Error("You can only invite friends.");
      if ((await memberIds(pa.id)).length >= MAX_PARTY) throw new Error("Party is full (max 4).");
      await pool.query(
        `insert into party_invites (party_id, from_id, to_id) values ($1, $2, $3)
         on conflict (party_id, to_id) do update set created_at = now(), from_id = excluded.from_id`,
        [pa.id, uid, id],
      );
      const me = await ensureProfile(uid);
      await notify([id], { k: "invite", partyId: String(pa.id), fromName: me.display_name });
      return;
    }
    case "party.join": {
      const partyId = String(msg.partyId ?? "");
      const token = msg.token ? String(msg.token) : null;
      let viaLink = false;
      if (token) {
        const t = await pool.query("select host_id from parties where id = $1 and token = $2", [partyId, token]);
        if (!t.rowCount) throw new Error("That invite link is invalid or the party has closed.");
        viaLink = true;
        // a shared party link doubles as a friend invite from the host
        const host = t.rows[0].host_id as string;
        if (host !== uid) {
          for (const [a, b] of [[uid, host], [host, uid]]) {
            await pool.query(
              `insert into friends (user_id, friend_id, status) values ($1, $2, 'accepted')
               on conflict (user_id, friend_id) do update set status = 'accepted'`,
              [a, b],
            );
          }
          await notify([uid, host], { k: "social" });
        }
      } else {
        const inv = await pool.query("select 1 from party_invites where party_id = $1 and to_id = $2", [partyId, uid]);
        if (!inv.rowCount) throw new Error("That invite has expired.");
      }
      const cur = await partyOf(uid);
      if (cur && String(cur.id) !== partyId) await leaveParty(uid);
      const client = await pool.connect();
      try {
        await client.query("begin");
        const pa = await client.query("select * from parties where id = $1 for update", [partyId]);
        // joining mid-match is fine: the newcomer asks the host to drop them in (M2)
        if (!pa.rows[0] || !["open", "in_game"].includes(pa.rows[0].status)) throw new Error("That party is no longer open.");
        const n = await client.query("select count(*)::int as n from party_members where party_id = $1", [partyId]);
        if (n.rows[0].n >= MAX_PARTY) throw new Error("Party is full (max 4).");
        await client.query(
          "insert into party_members (party_id, user_id, look) values ($1, $2, $3) on conflict do nothing",
          [partyId, uid, msg.look ?? {}],
        );
        await client.query("delete from party_invites where party_id = $1 and to_id = $2", [partyId, uid]);
        await client.query("commit");
      } catch (e) {
        await client.query("rollback");
        throw e;
      } finally {
        client.release();
      }
      await notify(await memberIds(partyId), { k: "party" });
      await notify([uid], { k: "social" });
      if (viaLink) {
        const me = await ensureProfile(uid);
        await notify((await memberIds(partyId)).filter((x) => x !== uid), { k: "toast", msg: `${me.display_name} joined the party` });
      }
      return;
    }
    case "party.decline": {
      await pool.query("delete from party_invites where party_id = $1 and to_id = $2", [String(msg.partyId ?? ""), uid]);
      await notify([uid], { k: "social" });
      return;
    }
    case "party.leave":
      await leaveParty(uid);
      await notify(await friendIds(uid), { k: "social" });
      return;
    case "party.ready": {
      const pa = await partyOf(uid);
      if (!pa) return;
      await pool.query("update party_members set ready = $3 where party_id = $1 and user_id = $2", [pa.id, uid, !!msg.ready]);
      await notify(await memberIds(pa.id), { k: "party" });
      return;
    }
    // Home Base: where you are, relayed to your party (a few times a second)
    case "hub.pos": {
      const pa = await partyOf(uid);
      if (!pa) return;
      const num = (v: unknown) => (Number.isFinite(Number(v)) ? Math.round(Number(v) * 100) / 100 : 0);
      const look = Object.fromEntries(
        ["skin", "hat", "gun"].map((k) => [k, String(msg.look?.[k] ?? "").slice(0, 20)]),
      );
      const me = await pool.query("select display_name from profiles where user_id = $1", [uid]);
      const ids = (await memberIds(pa.id)).filter((id) => id !== uid);
      await notify(ids, { k: "hub", from: uid, name: me.rows[0]?.display_name ?? "?", x: num(msg.x), y: num(msg.y), aim: num(msg.aim), look });
      return;
    }
    case "party.look": {
      const pa = await partyOf(uid);
      if (!pa) return;
      const look = Object.fromEntries(
        ["skin", "hat", "tracer", "gun", "perk"].map((k) => [k, String(msg.look?.[k] ?? "").slice(0, 20)]),
      );
      await pool.query("update party_members set look = $3 where party_id = $1 and user_id = $2", [pa.id, uid, look]);
      await notify(await memberIds(pa.id), { k: "party" });
      return;
    }
    case "party.diff": {
      const pa = await partyOf(uid);
      if (!pa || pa.host_id !== uid) throw new Error("Only the host can change the difficulty.");
      if (!DIFFS.includes(msg.diff)) return;
      await pool.query("update parties set diff = $2 where id = $1", [pa.id, msg.diff]);
      await notify(await memberIds(pa.id), { k: "party" });
      return;
    }
    case "party.mode": {
      const pa = await partyOf(uid);
      if (!pa || pa.host_id !== uid) throw new Error("Only the host can change the mode.");
      if (!["infinite", "campaign", "outbreak", "siege"].includes(msg.mode)) return;
      // campaign co-op plays one of the host's unlocked chapters
      let chapter = 0;
      if (msg.mode === "campaign") {
        const pr = await pool.query("select campaign from profiles where user_id = $1", [uid]);
        const unlocked = Math.max(0, Math.floor(Number(pr.rows[0]?.campaign?.unlocked) || 0));
        chapter = Math.max(0, Math.min(unlocked, Math.floor(Number(msg.chapter) || 0)));
      }
      await pool.query("update parties set mode = $2, chapter = $3 where id = $1", [pa.id, msg.mode, chapter]);
      await notify(await memberIds(pa.id), { k: "party" });
      return;
    }
    case "party.kick": {
      const pa = await partyOf(uid);
      if (!pa || pa.host_id !== uid) throw new Error("Only the host can kick.");
      const id = String(msg.id ?? "");
      if (id === uid) return;
      await pool.query("delete from party_members where party_id = $1 and user_id = $2", [pa.id, id]);
      await notify([id], { k: "party" });
      await notify([id], { k: "toast", msg: "You were removed from the party" });
      await notify(await memberIds(pa.id), { k: "party" });
      return;
    }
    case "party.start": {
      const pa = await partyOf(uid);
      if (!pa || pa.host_id !== uid) throw new Error("Only the host can start.");
      const m = await pool.query("select user_id, ready from party_members where party_id = $1", [pa.id]);
      if (m.rows.some((r) => !r.ready)) throw new Error("Everyone needs to be ready.");
      const seed = Math.floor(Math.random() * 2 ** 31);
      await pool.query("update parties set status = 'in_game', seed = $2 where id = $1", [pa.id, seed]);
      await pool.query("delete from party_invites where party_id = $1", [pa.id]);
      await notify(m.rows.map((r) => r.user_id), { k: "start" });
      return;
    }
    case "party.end": {
      const pa = await partyOf(uid);
      if (!pa || pa.host_id !== uid) return;
      await pool.query("update parties set status = 'open' where id = $1", [pa.id]);
      await pool.query("update party_members set ready = (user_id = $2) where party_id = $1", [pa.id, uid]);
      await notify(await memberIds(pa.id), { k: "party" });
      return;
    }

    case "chat": {
      const pa = await partyOf(uid);
      if (!pa) throw new Error("Join a party to chat.");
      const text = cleanChat(msg.text);
      if (!text) return;
      if (!chatAllowed(uid)) throw new Error("Slow down a little!");
      const me = await ensureProfile(uid);
      // members who blocked the sender don't receive it
      const r = await pool.query(
        `select m.user_id from party_members m where m.party_id = $1
           and not exists (select 1 from blocks b where b.user_id = m.user_id and b.blocked_id = $2)`,
        [pa.id, uid],
      );
      await notify(r.rows.map((x) => x.user_id), { k: "chat", from: uid, name: me.display_name, text, ts: Date.now() });
      return;
    }
    case "party.voice": {
      const pa = await partyOf(uid);
      if (!pa) return;
      await pool.query("update party_members set voice = $3 where party_id = $1 and user_id = $2", [pa.id, uid, !!msg.on]);
      await notify(await memberIds(pa.id), { k: "party" });
      return;
    }
    case "block": {
      const id = String(msg.id ?? "");
      if (!id || id === uid) return;
      await pool.query("insert into blocks (user_id, blocked_id) values ($1, $2) on conflict do nothing", [uid, id]);
      await notify([uid], { k: "social" });
      await notify([uid], { k: "toast", msg: "Player blocked. You won't see their messages or hear them." });
      return;
    }
    case "unblock": {
      await pool.query("delete from blocks where user_id = $1 and blocked_id = $2", [uid, String(msg.id ?? "")]);
      await notify([uid], { k: "social" });
      return;
    }
    case "report": {
      const id = String(msg.id ?? "");
      if (!id || id === uid) return;
      const recent = await pool.query(
        "select count(*)::int as n from reports where reporter_id = $1 and created_at > now() - interval '1 hour'",
        [uid],
      );
      if (recent.rows[0].n >= 10) throw new Error("You've sent a lot of reports. Try again later.");
      await pool.query(
        "insert into reports (reporter_id, reported_id, reason, message) values ($1, $2, $3, $4)",
        [uid, id, String(msg.reason ?? "").slice(0, 40), String(msg.message ?? "").slice(0, 300)],
      );
      await notify([uid], { k: "toast", msg: "Thanks. We've received your report." });
      return;
    }

    case "signal": {
      // WebRTC offer/answer/ICE, only between members of the same party
      const to = String(msg.to ?? "");
      const data = msg.data;
      if (JSON.stringify(data ?? null).length > 16_000) throw new Error("signal too large");
      const pa = await partyOf(uid);
      if (!pa || !(await memberIds(pa.id)).includes(to)) return;
      await notify([to], { k: "signal", from: uid, data });
      return;
    }
  }
}

// ---------------------------------------------------------------- route
type Env = { Variables: { userId: string } };
export const social = new Hono<Env>();

social.use("/ws", async (c, next) => {
  // browsers can't set headers on a WebSocket: the JWT comes as ?token=
  const uid = await verifyToken(c.req.query("token"));
  if (!uid) return c.text("Unauthorized", 401);
  await ensureProfile(uid);
  c.set("userId", uid);
  await next();
});

social.get(
  "/ws",
  upgradeWebSocket((c) => {
    const uid = c.get("userId");
    let budget = 40; // simple per-socket rate limit (messages per second)
    const refill = setInterval(() => (budget = 40), 1000);
    return {
      async onOpen(_evt, ws) {
        const raw = ws.raw as WS;
        if (!sockets.has(uid)) sockets.set(uid, new Set());
        sockets.get(uid)!.add(raw);
        await touchPresence(uid, true);
        const me = await ensureProfile(uid);
        sendWs(raw, { t: "hello", me: { id: uid, name: me.display_name, code: me.friend_code } });
        sendWs(raw, { t: "social", ...(await socialState(uid)) });
        sendWs(raw, { t: "party", party: await partyState(uid) });
        await notify(await friendIds(uid), { k: "social" });
      },
      async onMessage(evt, ws) {
        const raw = ws.raw as WS;
        if (--budget < 0) return;
        let msg: any;
        try {
          msg = JSON.parse(typeof evt.data === "string" ? evt.data : "");
        } catch {
          return;
        }
        try {
          await handle(uid, msg);
        } catch (e: any) {
          sendWs(raw, { t: "error", msg: e?.message ?? "Something went wrong", req: msg?.t });
        }
      },
      async onClose(_evt, ws) {
        clearInterval(refill);
        const set = sockets.get(uid);
        set?.delete(ws.raw as WS);
        if (set && set.size === 0) {
          sockets.delete(uid);
          await touchPresence(uid, false).catch(() => {});
          await notify(await friendIds(uid), { k: "social" }).catch(() => {});
          const pa = await partyOf(uid).catch(() => null);
          if (pa) {
            await pool.query("update party_members set voice = false where party_id = $1 and user_id = $2", [pa.id, uid]).catch(() => {});
            await notify(await memberIds(pa.id), { k: "party" }).catch(() => {});
          }
        }
      },
    };
  }),
);
