import { Hono } from "hono";
import { cors } from "hono/cors";

type Bindings = {
  SCORES: KVNamespace;
};

interface ScoreEntry {
  player: string;
  score: number;
  level: number;
  combo_max: number;
  date: string;
}

interface SubmitRequest {
  player: string;
  score: number;
  level: number;
  combo_max: number;
  mode: "daily" | "freeplay";
  date?: string;
}

const MAX_SCORES_PER_KEY = 100;
const RATE_LIMIT_SECONDS = 60;

const PLAYER_RE = /^[a-zA-Z0-9_]{1,20}$/;
const DATE_RE = /^\d{4}-\d{2}-\d{2}$/;

const app = new Hono<{ Bindings: Bindings }>();

// ---------------------------------------------------------------------------
// CORS
// ---------------------------------------------------------------------------
app.use("*", cors({ origin: "*" }));

// ---------------------------------------------------------------------------
// POST /api/scores
// ---------------------------------------------------------------------------
app.post("/api/scores", async (c) => {
  // --- Rate limiting by IP ---
  const ip = c.req.header("cf-connecting-ip") ?? c.req.header("x-forwarded-for") ?? "unknown";
  const rateLimitKey = `ratelimit:${ip}`;
  const existing = await c.env.SCORES.get(rateLimitKey);
  if (existing !== null) {
    return c.json({ error: "Rate limited. Try again in a few seconds." }, 429);
  }

  // --- Parse & validate body ---
  let body: SubmitRequest;
  try {
    body = await c.req.json<SubmitRequest>();
  } catch {
    return c.json({ error: "Invalid JSON body." }, 400);
  }

  const { player, score, level, combo_max, mode, date } = body;

  if (typeof player !== "string" || !PLAYER_RE.test(player)) {
    return c.json({ error: "player must be 1-20 alphanumeric/underscore characters." }, 400);
  }
  if (!Number.isInteger(score) || score < 1 || score > 999999) {
    return c.json({ error: "score must be a positive integer, max 999999." }, 400);
  }
  if (!Number.isInteger(level) || level < 1 || level > 999) {
    return c.json({ error: "level must be a positive integer, max 999." }, 400);
  }
  if (!Number.isInteger(combo_max) || combo_max < 0 || combo_max > 999) {
    return c.json({ error: "combo_max must be a non-negative integer, max 999." }, 400);
  }
  if (mode !== "daily" && mode !== "freeplay") {
    return c.json({ error: 'mode must be "daily" or "freeplay".' }, 400);
  }
  if (mode === "daily") {
    if (typeof date !== "string" || !DATE_RE.test(date)) {
      return c.json({ error: "date is required for daily mode (YYYY-MM-DD)." }, 400);
    }
  }

  // Resolve the effective date: use submitted date for daily, today's UTC date for freeplay
  const effectiveDate =
    mode === "daily" ? date! : new Date().toISOString().slice(0, 10);

  // --- Set rate limit AFTER validation passes ---
  await c.env.SCORES.put(rateLimitKey, "1", { expirationTtl: RATE_LIMIT_SECONDS });

  // --- Build the score entry ---
  const entry: ScoreEntry = {
    player,
    score,
    level,
    combo_max,
    date: effectiveDate,
  };

  // --- Determine KV key ---
  const kvKey = mode === "daily" ? `daily:${effectiveDate}` : "freeplay";

  // --- Read existing scores, insert, sort, trim ---
  const raw = await c.env.SCORES.get(kvKey);
  let scores: ScoreEntry[] = raw ? JSON.parse(raw) : [];

  scores.push(entry);
  scores.sort((a, b) => b.score - a.score);
  scores = scores.slice(0, MAX_SCORES_PER_KEY);

  await c.env.SCORES.put(kvKey, JSON.stringify(scores));

  // --- Compute rank (1-based, or null if trimmed out) ---
  const rank = scores.findIndex(
    (s) =>
      s.player === entry.player &&
      s.score === entry.score &&
      s.level === entry.level &&
      s.combo_max === entry.combo_max &&
      s.date === entry.date
  );

  return c.json({
    rank: rank === -1 ? null : rank + 1,
    total: scores.length,
  });
});

// ---------------------------------------------------------------------------
// GET /api/leaderboard/daily?date=YYYY-MM-DD&limit=N
// ---------------------------------------------------------------------------
app.get("/api/leaderboard/daily", async (c) => {
  const date = c.req.query("date");
  if (!date || !DATE_RE.test(date)) {
    return c.json({ error: "date query param required (YYYY-MM-DD)." }, 400);
  }

  const limit = parseLimit(c.req.query("limit"));
  if (limit === null) {
    return c.json({ error: "limit must be a positive integer, max 100." }, 400);
  }

  const kvKey = `daily:${date}`;
  const raw = await c.env.SCORES.get(kvKey);
  const scores: ScoreEntry[] = raw ? JSON.parse(raw) : [];

  return c.json({ scores: scores.slice(0, limit) });
});

// ---------------------------------------------------------------------------
// GET /api/leaderboard/freeplay?limit=N
// ---------------------------------------------------------------------------
app.get("/api/leaderboard/freeplay", async (c) => {
  const limit = parseLimit(c.req.query("limit"));
  if (limit === null) {
    return c.json({ error: "limit must be a positive integer, max 100." }, 400);
  }

  const raw = await c.env.SCORES.get("freeplay");
  const scores: ScoreEntry[] = raw ? JSON.parse(raw) : [];

  return c.json({ scores: scores.slice(0, limit) });
});

// ---------------------------------------------------------------------------
// 404 fallback
// ---------------------------------------------------------------------------
app.all("*", (c) => c.json({ error: "Not found." }, 404));

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------
function parseLimit(raw: string | undefined): number | null {
  if (raw === undefined) return 10; // default
  const n = Number(raw);
  if (!Number.isInteger(n) || n < 1 || n > 100) return null;
  return n;
}

export default app;
