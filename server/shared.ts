// Shared server pieces: Postgres pool, Neon Auth JWT verification, profiles.
import { Pool } from "pg";
import { createRemoteJWKSet, jwtVerify } from "jose";
import { attachDatabasePool } from "@neon/functions";

export const pool = new Pool({ connectionString: process.env.DATABASE_URL, max: 5 });
attachDatabasePool(pool);

const jwks = createRemoteJWKSet(new URL(process.env.NEON_AUTH_JWKS_URL!));
const issuer = new URL(process.env.NEON_AUTH_BASE_URL!).origin;

/** Returns the user id for a valid Neon Auth JWT, otherwise null. */
export async function verifyToken(token: string | null | undefined): Promise<string | null> {
  if (!token) return null;
  try {
    const { payload } = await jwtVerify(token, jwks, { issuer });
    return payload.sub ?? null;
  } catch {
    return null;
  }
}

export const cleanName = (s: unknown) =>
  String(s ?? "")
    .replace(/[^\p{L}\p{N} _.\-]/gu, "")
    .trim()
    .slice(0, 16);

const newCode = () => Math.random().toString(36).slice(2, 8).toUpperCase().padEnd(6, "X");

export async function ensureProfile(userId: string) {
  const existing = await pool.query("select * from profiles where user_id = $1", [userId]);
  if (existing.rows[0]?.friend_code) return existing.rows[0];
  if (existing.rows[0]) {
    const { rows } = await pool.query(
      "update profiles set friend_code = $2 where user_id = $1 and friend_code is null returning *",
      [userId, newCode()],
    );
    return rows[0] ?? existing.rows[0];
  }
  const u = await pool.query(`select name, email from neon_auth."user" where id = $1`, [userId]);
  const base = u.rows[0]?.name || String(u.rows[0]?.email || "Survivor").split("@")[0];
  const name = cleanName(base) || "Survivor";
  const { rows } = await pool.query(
    `insert into profiles (user_id, display_name, friend_code) values ($1, $2, $3)
     on conflict (user_id) do update set user_id = excluded.user_id
     returning *`,
    [userId, name, newCode()],
  );
  return rows[0];
}
