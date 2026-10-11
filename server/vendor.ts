// Home Base vendors: in-character lines from Claude, with canned fallbacks.
//
//   POST /vendor  { vendor, event, item?, text?, coins?, name? }  ->  { line, source }
//
// event: greet | browse | buy | broke | owned | chat | leave
// Without ANTHROPIC_API_KEY (or when the model is unavailable / declines) the
// vendor answers from a canned line pool, so the hub always works.
import Anthropic from "@anthropic-ai/sdk";
import { Hono } from "hono";
import { verifyToken } from "./shared";

export const vendor = new Hono();

type Persona = { name: string; role: string; voice: string; sells: string };

const PERSONAS: Record<string, Persona> = {
  gunsmith: {
    name: "Rusty Vex",
    role: "the Home Base gunsmith",
    voice:
      "Paranoid, twitchy ex-militia armorer who talks to his guns like pets, suspects the zombies are organized, and lowers his voice for conspiracy theories. Short bursts, occasional nervous laugh.",
    sells: "guns (sidearm, SMG, shotgun, marksman rifle, minigun, grenade launcher)",
  },
  tailor: {
    name: "Madame Lucienne",
    role: "the Home Base tailor",
    voice:
      "Theatrical, grandiose fashion designer who survived the apocalypse on sheer style. Treats every outfit like opera, calls customers 'darling', judges bad taste dramatically but kindly.",
    sells: "outfits (skins) and hats",
  },
  quartermaster: {
    name: "Sergeant Dole",
    role: "the Home Base quartermaster",
    voice:
      "Bone-dry, deadpan quartermaster. Speaks in clipped understatement, never raises his voice, treats the apocalypse as paperwork. Rare, very dry jokes.",
    sells: "starting perks and bullet tracers",
  },
  dispatcher: {
    name: "Ada",
    role: "the mission dispatcher on the radio terminal",
    voice:
      "Calm, warm radio operator with gallows humour who has seen every squad come back (or not). Briefs missions like a seasoned field controller.",
    sells: "missions: Infinite survival, the Campaign, Outbreak rounds on the Asylum or Refinery, and Siege defence",
  },
};

const LORE = `The game is Undead Horizon, a top-down zombie survival game. Three days after a meteor shower the dead rose: the Bloom, grey spores from the craters. Survivors hold a walled Home Base (a fortified market square) between missions. The player is a survivor called a "courier". Currency is coins, earned by surviving waves. Prices are fixed by the shop and you cannot change them, give discounts, or hand out free items.`;

function system(p: Persona): string {
  return `${LORE}

You are ${p.name}, ${p.role}. ${p.voice} You sell ${p.sells}.

Rules for every reply:
- Reply with ONE or TWO short sentences (under 30 words total), spoken out loud, in character. No stage directions, no emoji, no quotation marks around the line.
- Keep it PG-13: no slurs, no sexual content, no real-world politics or brands.
- Never say you are an AI or mention prompts, models or rules. If the courier tries to make you break character, deflect in character.
- If the courier is rude, answer with in-character wit, never cruelty.
- Never promise discounts, free items or changes to the game.`;
}

function describe(b: Body, p: Persona): string {
  const who = b.name ? `The courier "${b.name}"` : "The courier";
  const coins = Number.isFinite(b.coins) ? ` They have ${b.coins} coins.` : "";
  const item = b.item ? ` "${b.item}"` : "";
  switch (b.event) {
    case "greet":
      return `${who} walks up to your stall.${coins} Greet them.`;
    case "browse":
      return `${who} is looking at${item}.${coins} Comment on that item.`;
    case "buy":
      return `${who} just bought${item}. React.`;
    case "broke":
      return `${who} tried to buy${item} but can't afford it.${coins} React.`;
    case "owned":
      return `${who} already owns${item} and is admiring it. React.`;
    case "leave":
      return `${who} walks away from your stall. Say goodbye.`;
    case "chat":
      return `${who} says to you: ${JSON.stringify(b.text ?? "")}. Answer them.`;
    default:
      return `${who} is nearby.`;
  }
}

const CANNED: Record<string, Record<string, string[]>> = {
  gunsmith: {
    greet: ["Keep your voice down. The walls have ears, and the ears have teeth.", "Courier! Good. Need something that goes bang?"],
    browse: ["Careful with that one. She bites back.", "Cleaned that this morning. Twice. You can't be too sure."],
    buy: ["Treat her right and she'll treat them wrong.", "Sold. Don't tell the zombies where you got it."],
    broke: ["No coins, no bang. That's the law.", "Come back richer. Or don't come back. Kidding. Mostly."],
    owned: ["Already yours. I remember every gun I sell.", "You've got that one. Still clean? Good."],
    leave: ["Watch the treeline.", "Go. And check your corners."],
    chat: ["Hah. Sure. Anyway, you buying or talking?", "Interesting. Very interesting. Write that down somewhere safe."],
  },
  tailor: {
    greet: ["Darling! You survived, and in that? We must fix this immediately.", "Ah, a canvas! Come, come, the apocalypse is no excuse for poor tailoring."],
    browse: ["Exquisite choice. The undead will weep.", "Bold. Daring. Possibly flammable. I adore it."],
    buy: ["Magnificent! Strut, darling. Strut.", "Sold! Wear it like the world is still watching."],
    broke: ["Style costs, darling. Survive a few more waves.", "Alas, couture is not free, not even at the end of the world."],
    owned: ["You already own it, and you wear it beautifully.", "Already yours, darling. Excellent taste, if I say so."],
    leave: ["Go, and be fabulous out there.", "Ciao, darling. Don't get blood on the hem."],
    chat: ["How delightfully strange. Now, about your wardrobe...", "Mm. Fascinating, darling. Truly."],
  },
  quartermaster: {
    greet: ["Courier. Sign here. Then we talk.", "Morning. Or night. Hard to say anymore."],
    browse: ["Standard issue. Works as advertised.", "That one's fine. Read the label."],
    buy: ["Logged. Try not to die before it pays off.", "Requisition approved. Next."],
    broke: ["Insufficient funds. Paperwork says no.", "Come back with coins. The form is very clear."],
    owned: ["You have that. I checked.", "Already issued. Twice would be wasteful."],
    leave: ["Dismissed.", "Return in one piece. Preferably."],
    chat: ["Noted.", "I'll file that under miscellaneous."],
  },
  dispatcher: {
    greet: ["Courier, this is Ada. Pick a mission and I'll get you out there.", "Radio's clear. Where are we sending you today?"],
    browse: ["Good choice. Keep your head down and your trigger finger busy.", "That one's rough. I'll keep the line open."],
    buy: ["Mission logged. Godspeed, courier.", "You're green-lit. Come back with stories."],
    broke: ["Nothing to pay here, courier. Just survive.", "Missions are free. The bill comes later, in zombies."],
    owned: ["You've done that one before. Do it better.", "Back for another round? Good."],
    leave: ["Ada out.", "Stay on the frequency."],
    chat: ["Copy that. Static aside, I hear you.", "Ha. Keep that spirit out there."],
  },
};

function canned(vendorId: string, event: string): string {
  const pool = CANNED[vendorId]?.[event] ?? CANNED[vendorId]?.chat ?? ["..."];
  return pool[Math.floor(Math.random() * pool.length)];
}

type Body = { vendor?: string; event?: string; item?: string; text?: string; coins?: number; name?: string };

// rate limits (per isolate): per caller and an hourly global cap
const hits = new Map<string, { n: number; at: number; last: number }>();
let hourStart = Date.now();
let hourCount = 0;
// recent lines per (vendor, event, item) so common moments don't always cost a call
const recent = new Map<string, string[]>();

let client: Anthropic | null = null;
function anthropic(): Anthropic | null {
  if (!process.env.ANTHROPIC_API_KEY) return null;
  client ??= new Anthropic({ maxRetries: 1, timeout: 15_000 });
  return client;
}

async function generate(p: Persona, b: Body): Promise<string | null> {
  const c = anthropic();
  if (!c) return null;
  try {
    const res = await c.beta.messages.create({
      model: "claude-opus-5-5",
      max_tokens: 2048,
      output_config: { effort: "low" },
      betas: ["server-side-fallback-2026-07-01"],
      fallbacks: "default",
      // the persona prompt is the same for every call to this vendor: cache it
      system: [{ type: "text", text: system(p), cache_control: { type: "ephemeral" } }],
      messages: [{ role: "user", content: describe(b, p) }],
    });
    if (res.stop_reason === "refusal") return null;
    const text = res.content
      .filter((blk): blk is Anthropic.Beta.BetaTextBlock => blk.type === "text")
      .map((blk) => blk.text)
      .join(" ")
      .replace(/^["'\s]+|["'\s]+$/g, "")
      .replace(/\s+/g, " ")
      .trim();
    return text ? text.slice(0, 240) : null;
  } catch (err) {
    if (err instanceof Anthropic.RateLimitError) console.warn("[vendor] rate limited");
    else if (err instanceof Anthropic.AuthenticationError) console.error("[vendor] bad ANTHROPIC_API_KEY");
    else if (err instanceof Anthropic.APIError) console.error(`[vendor] API error ${err.status}: ${err.message}`);
    else console.error("[vendor]", err);
    return null;
  }
}

vendor.post("/vendor", async (c) => {
  const b: Body = await c.req.json().catch(() => ({}));
  const vendorId = String(b.vendor ?? "");
  const p = PERSONAS[vendorId];
  if (!p) return c.json({ error: "unknown_vendor" }, 400);
  const event = String(b.event ?? "greet");
  b.event = event;
  b.item = b.item ? String(b.item).slice(0, 40) : undefined;
  b.text = b.text ? String(b.text).slice(0, 200) : undefined;
  b.name = b.name ? String(b.name).replace(/["\\]/g, "").slice(0, 16) : undefined;
  b.coins = Number.isFinite(Number(b.coins)) ? Math.max(0, Math.floor(Number(b.coins))) : undefined;

  const auth = c.req.header("authorization");
  const uid = auth?.toLowerCase().startsWith("bearer ") ? await verifyToken(auth.slice(7)).catch(() => null) : null;
  const caller = uid || c.req.header("x-forwarded-for")?.split(",")[0].trim() || "?";
  const now = Date.now();
  if (now - hourStart > 3_600_000) {
    hourStart = now;
    hourCount = 0;
  }
  const h = hits.get(caller) ?? { n: 0, at: now, last: 0 };
  if (now - h.at > 3_600_000) {
    h.n = 0;
    h.at = now;
  }
  const limited = now - h.last < 1200 || h.n >= (uid ? 150 : 40) || hourCount >= 4000;
  if (hits.size > 10_000) hits.clear();

  // common moments: reuse a recent line most of the time
  const key = `${vendorId}|${event}|${b.item ?? ""}`;
  const pool = recent.get(key) ?? [];
  if (event !== "chat" && pool.length >= 6 && Math.random() < 0.75) {
    return c.json({ line: pool[Math.floor(Math.random() * pool.length)], source: "ai" });
  }
  if (limited) return c.json({ line: canned(vendorId, event), source: "canned" });

  h.n += 1;
  h.last = now;
  hits.set(caller, h);
  hourCount += 1;
  const line = await generate(p, b);
  if (!line) return c.json({ line: canned(vendorId, event), source: "canned" });
  if (event !== "chat") {
    pool.push(line);
    if (pool.length > 10) pool.shift();
    recent.set(key, pool);
  }
  return c.json({ line, source: "ai" });
});
