// Cloudflare Worker (free tier) that counts unique installs. Bind a KV namespace named STATS.
// POST /ping  {id, v, os}  -> stores one key per install id, refreshed on every ping
// GET  /stats?key=YOUR_SECRET -> {total, active_7d, active_30d, by_version}
export default {
  async fetch(req, env) {
    const url = new URL(req.url);
    if (req.method === "POST" && url.pathname === "/ping") {
      let b; try { b = await req.json(); } catch { return new Response("bad", { status: 400 }); }
      if (!/^[0-9a-f-]{36}$/.test(b.id || "")) return new Response("bad", { status: 400 });
      await env.STATS.put("i:" + b.id, JSON.stringify({ v: String(b.v).slice(0, 20), os: String(b.os).slice(0, 10), t: Date.now() }));
      return new Response("ok");
    }
    if (req.method === "GET" && url.pathname === "/stats" && url.searchParams.get("key") === env.ADMIN_KEY) {
      let total = 0, d7 = 0, d30 = 0, by = {}, cursor;
      do {
        const page = await env.STATS.list({ prefix: "i:", cursor });
        for (const k of page.keys) {
          const r = JSON.parse(await env.STATS.get(k.name)); total++;
          const age = (Date.now() - r.t) / 864e5;
          if (age <= 7) d7++; if (age <= 30) d30++; by[r.v] = (by[r.v] || 0) + 1;
        }
        cursor = page.list_complete ? undefined : page.cursor;
      } while (cursor);
      return Response.json({ total, active_7d: d7, active_30d: d30, by_version: by });
    }
    return new Response("not found", { status: 404 });
  },
};
