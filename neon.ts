import { defineConfig } from "@neon/config/v1";

export default defineConfig({
  // Managed Better Auth (accounts live in the neon_auth schema)
  auth: true,
  functions: {
    // Game API: profiles, shop, saves, runs and the global leaderboard
    api: {
      name: "undead horizon api",
      source: "server/index.ts",
      // TURN relay for co-op (GET /turn); values live in the gitignored .env.local
      env: {
        METERED_APP: process.env.METERED_APP!,
        METERED_SECRET_KEY: process.env.METERED_SECRET_KEY!,
      },
    },
  },
});
