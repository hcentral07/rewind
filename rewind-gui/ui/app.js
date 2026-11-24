/* Rewind v0.5.0 — UI
 * Plain-JS React (no JSX) targeting the vendored react.production.min.js.
 * Invariant: runs against window.__TAURI__.core.invoke when present, otherwise
 * falls back to a demo backend so the UI is fully explorable in a browser.
 */
(function () {
  "use strict";

  const { useState, useEffect, useRef, useCallback, useMemo } = React;
  const h = React.createElement;

  /* ---------------------------------------------------------------- backend */

  const tauriInvoke =
    window.__TAURI__ && window.__TAURI__.core
      ? window.__TAURI__.core.invoke
      : null;

  const demoNow = Date.now();
  const iso = (minsAgo) => new Date(demoNow - minsAgo * 60000).toISOString();
  const DEMO_ROOT = "C:\\projects\\demo";
  const DEMO_FILE = DEMO_ROOT + "\\src\\main.rs";

  // Mutable demo state so restore/diff are actually explorable in the browser.
  const demoState = {
    versions: [
      { id: 3, number: 3, path: DEMO_FILE, hash: "7f9a2c1d00aa", size: 2210,
        created_at: iso(2), lines_added: 14, lines_deleted: 3,
        summary: "Modified: +14 -3 lines (2.2 KB)", is_current: true },
      { id: 2, number: 2, path: DEMO_FILE, hash: "3e4b9d0f11bb", size: 1900,
        created_at: iso(49), lines_added: 28, lines_deleted: 12,
        summary: "Modified: +28 -12 lines (1.9 KB)", is_current: false },
      { id: 1, number: 1, path: DEMO_FILE, hash: "8a1c5e4a22cc", size: 1500,
        created_at: iso(180), lines_added: 45, lines_deleted: 0,
        summary: "Created: 45 lines (1.5 KB)", is_current: false },
    ],
  };

  async function demo(cmd, args) {
    switch (cmd) {
      case "get_status":
        return {
          data_dir: "C:\\Users\\you\\AppData\\Local\\Rewind",
          total_versions: demoState.versions.length,
          total_files: 1,
          watches: [{ path: DEMO_ROOT, added_at: iso(200),
                      versions: demoState.versions.length, files: 1 }],
        };
      case "pick_folder":
      case "watch_folder":
        return DEMO_ROOT;
      case "unwatch_folder":
      case "reveal_path":
        return null;
      case "restore_version": {
        const v = demoState.versions.find((x) => x.id === args.versionId);
        if (v) {
          demoState.versions.forEach((x) => { x.is_current = x.id === v.id; });
        }
        return null;
      }
      case "list_files":
        return [{ path: DEMO_FILE, name: "src\\main.rs",
                  versions: demoState.versions.length, last_change: iso(2) }];
      case "get_history":
        return demoState.versions;
      case "get_diff": {
        const v = demoState.versions.find((x) => x.id === args.versionId);
        if (!v || v.number === 1) {
          return [{ kind: "hunk", text: "Initial snapshot — nothing to diff against." }];
        }
        return [
          { kind: "ctx",  text: "fn main() {" },
          { kind: "del",  text: "    let (tx, rx) = std::sync::mpsc::channel();" },
          { kind: "add",  text: "    let (tx, rx) = tokio::sync::mpsc::unbounded_channel();" },
          { kind: "hunk", text: "... 12 unchanged lines ..." },
          { kind: "ctx",  text: "}" },
        ];
      }
      default:
        return null;
    }
  }

  const api = (cmd, args) => (tauriInvoke ? tauriInvoke(cmd, args) : demo(cmd, args));

  /* ---------------------------------------------------------------- helpers */

  const baseName = (p) => (p || "").split(/[\\/]/).filter(Boolean).pop() || p || "";

  const clamp = (n, lo, hi) => Math.max(lo, Math.min(hi, n));

  const trim = (s, n) => (s.length > n ? s.slice(0, n - 1) + "…" : s);

  function fmtSize(bytes) {
    const b = Math.max(0, Number(bytes) || 0);
    if (b < 1024) return b + " B";
    if (b < 1048576) return (b / 1024).toFixed(1) + " KB";
    if (b < 1073741824) return (b / 1048576).toFixed(1) + " MB";
    return (b / 1073741824).toFixed(1) + " GB";
  }

  function relTime(isoStr) {
    const t = new Date(isoStr).getTime();
    if (isNaN(t)) return "";
    const s = Math.max(0, Math.floor((Date.now() - t) / 1000));
    if (s < 10) return "just now";
    if (s < 60) return s + "s ago";
    const m = Math.floor(s / 60);
    if (m < 60) return m + "m ago";
    const hr = Math.floor(m / 60);
    if (hr < 24) return hr + "h ago";
    const d = Math.floor(hr / 24);
    if (d < 30) return d + "d ago";
    return Math.floor(d / 30) + "mo ago";
  }

  function absTime(isoStr) {
    const d = new Date(isoStr);
    return isNaN(d)
      ? String(isoStr)
      : d.toLocaleString(undefined, {
          month: "short", day: "numeric",
          hour: "2-digit", minute: "2-digit", second: "2-digit",
        });
  }

  function normalizePath(p) {
    // trim, strip surrounding quotes (incl. smart quotes), drop trailing
    // slashes — but never touch a 3-char Windows root like "C:\".
    let s = String(p || "").trim();
    s = s.replace(/^["'“”‘’]|["'“”‘’]$/g, "").trim();
    if (s.length > 3) s = s.replace(/[\\/]+$/, "");
    return s;
  }

  // Copy with a Promise-aware path + legacy fallback. Returns Promise<boolean>.
  async function copyText(text) {
    try {
      if (navigator.clipboard && window.isSecureContext) {
        await navigator.clipboard.writeText(text);
        return true;
      }
    } catch (_) { /* fall through */ }
    try {
      const ta = document.createElement("textarea");
      ta.value = text;
      ta.setAttribute("readonly", "");
      ta.style.position = "fixed";
      ta.style.opacity = "0";
      document.body.appendChild(ta);
      ta.select();
      const ok = document.execCommand("copy");
      document.body.removeChild(ta);
      return ok;
    } catch (_) {
      return false;
    }
  }

  /* --------------------------------------------------------------- useInterval
   * Declarative interval hook that always calls the latest callback and
   * clears on unmount / dependency change. Avoids the copy-pasted
   * setInterval/clearInterval blocks and the stale-closure trap.
   */
  function useInterval(cb, ms) {
    const saved = useRef(cb);
    useEffect(() => { saved.current = cb; }, [cb]);
    useEffect(() => {
      if (ms == null) return;
      const id = setInterval(() => saved.current(), ms);
      return () => clearInterval(id);
    }, [ms]);
  }

  /* --------------------------------------------------------------- usePolling
   * Immediate fetch + interval poll, cancelled on unmount / key change.
   * Returns [data, { loading, error }].
   */
  function usePolling(fn, key, ms) {
    const [data, setData] = useState(null);
    const [state, setState] = useState({ loading: true, error: null });
    const saved = useRef(fn);
    useEffect(() => { saved.current = fn; }, [fn]);

    useEffect(() => {
      if (key == null) { setData(null); setState({ loading: false, error: null }); return; }
      let alive = true;
      const run = async () => {
        try {
          const d = await saved.current(key);
          if (alive) { setData(d); setState({ loading: false, error: null }); }
        } catch (e) {
          if (alive) setState((s) => ({ ...s, loading: false, error: e }));
        }
      };
      run();
      const id = setInterval(run, ms);
      return () => { alive = false; clearInterval(id); };
    }, [key, ms]);

    return [data, state];
  }

  /* ---------------------------------------------------------------- icons */

  // Icons are arrays of <path> `d` strings so multi-stroke glyphs work.
  const ICONS = {
    plus:    ["M5 12h14", "M12 5v14"],
    x:       ["M18 6 6 18", "M6 6l12 12"],
    up:      ["m5 12 7-7 7 7", "M12 19V5"],
    left:    ["m12 19-7-7 7-7", "M19 12H5"],
    folder:  ["M20 20a2 2 0 0 0 2-2V8a2 2 0 0 0-2-2h-7.9a2 2 0 0 1-1.69-.9L9.6 3.9A2 2 0 0 0 7.93 3H4a2 2 0 0 0-2 2v13a2 2 0 0 0 2 2Z"],
    restore: ["M3 12a9 9 0 1 0 9-9 9.75 9.75 0 0 0-6.74 2.74L3 8", "M3 3v5h5"],
    file:    ["M15 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V7Z", "M14 2v4a2 2 0 0 0 2 2h4"],
    diff:    ["M18 21a3 3 0 1 0 0-6 3 3 0 0 0 0 6Z", "M6 9a3 3 0 1 0 0-6 3 3 0 0 0 0 6Z", "M13 6h3a2 2 0 0 1 2 2v7", "M11 18H8a2 2 0 0 1-2-2V9"],
    history: ["M3 12a9 9 0 1 0 9-9 9.75 9.75 0 0 0-6.74 2.74L3 8", "M3 3v5h5", "M12 7v5l4 2"],
    home:    ["m3 9 9-7 9 7v11a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z", "M9 22V12h6v10"],
    check:   ["M20 6 9 17l-5-5"],
    copy:    ["M8 8h12v12H8z", "M4 16V4h12"],
    search:  ["m21 21-4.3-4.3", "M17 10a7 7 0 1 1-14 0 7 7 0 0 1 14 0Z"],
    shield:  ["M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10Z"],
  };

  function Ic({ n, c = "w-4 h-4" }) {
    const paths = ICONS[n] || [];
    return h("svg", {
      className: c, viewBox: "0 0 24 24", fill: "none",
      stroke: "currentColor", strokeWidth: "2",
      strokeLinecap: "round", strokeLinejoin: "round",
      "aria-hidden": "true", focusable: "false",
    }, paths.map((d, i) => h("path", { key: i, d })));
  }

  /* ---------------------------------------------------------- micro-components */

  function IconBtn({ n, onClick, title, cls = "", children }) {
    return h("button", {
      type: "button", onClick, title, "aria-label": title,
      className: "icon-btn " + cls,
    }, h(Ic, { n, c: "w-4 h-4" }), children);
  }

  function Pill({ tone = "emerald", children }) {
    const tones = {
      emerald: "bg-emerald-500/10 border-emerald-500/20 text-emerald-400",
      violet:  "bg-violet-600/20 border-violet-500/30 text-violet-300",
      neutral: "bg-ink-800 border-ink-700 text-zinc-300",
    };
    return h("span", {
      className: "inline-flex items-center gap-1.5 px-2.5 py-1 rounded-full border text-xs font-medium " + tones[tone],
    }, children);
  }

  function Logo({ c = "w-8 h-8" }) {
    const [bad, setBad] = useState(false);
    if (!bad) {
      return h("img", {
        src: "logo.png", alt: "Rewind",
        className: c + " rounded-lg object-cover flex-shrink-0",
        onError: () => setBad(true),
      });
    }
    return h("div", {
      className: c + " rounded-lg bg-violet-600/20 border border-violet-500/30 flex items-center justify-center text-violet-300 flex-shrink-0",
    }, h(Ic, { n: "history", c: "w-4 h-4" }));
  }

  /* ---------------------------------------------------------------- mascot */

  function Mascot() {
    const box = useRef(null);
    useEffect(() => {
      const el = box.current;
      if (!el) return;
      // Respect reduced motion: skip the whole animation loop.
      const reduced = window.matchMedia &&
        window.matchMedia("(prefers-reduced-motion: reduce)").matches;
      if (reduced) return;

      const mover = el.querySelector('[data-m="mover"]');
      const head  = el.querySelector('[data-m="head"]');
      const eyes  = el.querySelector('[data-m="eyes"]');
      const ears  = el.querySelector('[data-m="ears"]');
      let tx = 0, ty = 0, x = 0, y = 0, raf = 0, running = true;

      const onMove = (e) => {
        const r = el.getBoundingClientRect();
        const dx = e.clientX - (r.left + r.width / 2);
        const dy = e.clientY - (r.top + r.height / 2);
        tx = clamp(dx / (window.innerWidth * 0.45), -1, 1);
        ty = clamp(dy / (window.innerHeight * 0.45), -1, 1);
      };
      const onLeave = () => { tx = 0; ty = 0; };

      const tick = () => {
        x += (tx - x) * 0.1;
        y += (ty - y) * 0.1;
        mover.style.transform = `translate(${x * 22}px, ${y * 14}px)`;
        head.style.transform  = `translate(${x * 7}px, ${y * 5}px) rotate(${x * 5}deg)`;
        eyes.style.transform  = `translate(${x * 11}px, ${y * 9}px)`;
        ears.style.transform  = `translate(${x * 3}px, ${y * 2}px)`;
        if (running) raf = requestAnimationFrame(tick);
      };

      // Pause the loop when the tab is hidden — saves battery.
      const onVisibility = () => {
        if (document.hidden) {
          running = false;
          cancelAnimationFrame(raf);
        } else if (!running) {
          running = true;
          raf = requestAnimationFrame(tick);
        }
      };

      window.addEventListener("mousemove", onMove, { passive: true });
      document.addEventListener("mouseleave", onLeave);
      document.addEventListener("visibilitychange", onVisibility);
      raf = requestAnimationFrame(tick);

      return () => {
        running = false;
        window.removeEventListener("mousemove", onMove);
        document.removeEventListener("mouseleave", onLeave);
        document.removeEventListener("visibilitychange", onVisibility);
        cancelAnimationFrame(raf);
      };
    }, []);

    const BODY = "#1d1a38";
    return h("div", { ref: box, className: "relative" },
      h("div", { "data-m": "mover" },
        h("svg", { viewBox: "0 0 260 260", className: "w-56 h-56 overflow-visible rw-float" },
          h("defs", null,
            h("linearGradient", { id: "rwStroke", x1: "0", y1: "0", x2: "1", y2: "1" },
              h("stop", { offset: "0", stopColor: "#5b8cff" }),
              h("stop", { offset: "1", stopColor: "#8b5cf6" })),
            h("radialGradient", { id: "rwGlow" },
              h("stop", { offset: "0", stopColor: "#7c6cff", stopOpacity: ".38" }),
              h("stop", { offset: "1", stopColor: "#7c6cff", stopOpacity: "0" })),
            h("filter", { id: "rwBlur", x: "-80%", y: "-80%", width: "260%", height: "260%" },
              h("feGaussianBlur", { stdDeviation: "6" }))),
          h("circle", { cx: "130", cy: "140", r: "125", fill: "url(#rwGlow)" }),
          h("g", { "data-m": "head" },
            h("g", { "data-m": "ears" },
              h("polygon", { points: "52,112 22,140 52,168", fill: BODY, stroke: "url(#rwStroke)", strokeWidth: "9", strokeLinejoin: "round" }),
              h("polygon", { points: "208,112 238,140 208,168", fill: BODY, stroke: "url(#rwStroke)", strokeWidth: "9", strokeLinejoin: "round" })),
            h("rect", { x: "112", y: "30", width: "36", height: "56", rx: "18", fill: BODY, stroke: "url(#rwStroke)", strokeWidth: "9" }),
            h("rect", { x: "48", y: "62", width: "164", height: "152", rx: "44", fill: BODY, stroke: "url(#rwStroke)", strokeWidth: "9" }),
            h("rect", { x: "117", y: "55", width: "26", height: "14", fill: BODY }),
            h("g", { "data-m": "eyes" },
              h("g", { className: "rw-blink" },
                h("g", { filter: "url(#rwBlur)", opacity: ".7" },
                  h("rect", { x: "90", y: "116", width: "16", height: "46", rx: "8", fill: "url(#rwStroke)" }),
                  h("rect", { x: "154", y: "116", width: "16", height: "46", rx: "8", fill: "url(#rwStroke)" })),
                h("rect", { x: "90", y: "116", width: "16", height: "46", rx: "8", fill: "url(#rwStroke)" }),
                h("rect", { x: "154", y: "116", width: "16", height: "46", rx: "8", fill: "url(#rwStroke)" })))))));
  }

  /* ---------------------------------------------------------------- app */

  function App() {
    const [status, setStatus] = useState({ data_dir: "", total_versions: 0, total_files: 0, watches: [] });
    const [folder, setFolder] = useState(null);
    const [file, setFile] = useState(null);
    const [diff, setDiff] = useState(null);
    const [prompt, setPrompt] = useState("");
    const [busy, setBusy] = useState(false);
    const [toasts, setToasts] = useState([]);
    const [q, setQ] = useState("");
    const [confirmId, setConfirmId] = useState(null);

    const confirmTimer = useRef(null);
    const searchRef = useRef(null);
    const toastSeq = useRef(0);

    // Bumps every 15s so relative timestamps re-render. Value is unused on
    // purpose — the state change is the point.
    const [, setClock] = useState(0);

    /* --- toasts ----------------------------------------------------------- */
    const say = useCallback((msg, tone) => {
      const id = ++toastSeq.current;
      setToasts((t) => [...t.slice(-2), { id, msg: String(msg), tone }]);
      setTimeout(() => setToasts((t) => t.filter((x) => x.id !== id)), 2600);
    }, []);

    /* --- data loading ----------------------------------------------------- */
    const refreshStatus = useCallback(async () => {
      try { setStatus(await api("get_status")); } catch (_) { /* keep last */ }
    }, []);

    useInterval(refreshStatus, 2500);
    useInterval(() => setClock((n) => n + 1), 15000);

    useEffect(() => { refreshStatus(); }, [refreshStatus]);

    const [files, filesState] = usePolling(
      useCallback((f) => api("list_files", { folder: f }), []),
      folder, 2000
    );
    const fileList = files || [];

    // Keep `file` valid whenever the list changes.
    useEffect(() => {
      if (!fileList.length) { setFile(null); return; }
      setFile((cur) => (cur && fileList.some((x) => x.path === cur) ? cur : fileList[0].path));
    }, [fileList]);

    const [history, historyState] = usePolling(
      useCallback((p) => api("get_history", { path: p }), []),
      file, 2000
    );
    const hist = history || [];

    /* --- actions ---------------------------------------------------------- */
    const watchFolder = useCallback(async (p) => {
      const clean = normalizePath(p);
      if (!clean) return;
      setBusy(true);
      try {
        const key = await api("watch_folder", { path: clean });
        setFolder(key);
        setFile(null);
        setPrompt("");
        await refreshStatus();
        say("Watching " + baseName(key));
      } catch (e) {
        say(String(e));
      } finally {
        setBusy(false);
      }
    }, [refreshStatus, say]);

    const browse = useCallback(async () => {
      try {
        const p = await api("pick_folder");
        if (p) await watchFolder(p);
      } catch (e) { say(String(e)); }
    }, [watchFolder, say]);

    const unwatch = useCallback(async (path) => {
      try {
        await api("unwatch_folder", { path });
        if (folder === path) { setFolder(null); setFile(null); }
        await refreshStatus();
        say("Stopped watching " + baseName(path) + " (history kept)");
      } catch (e) { say(String(e)); }
    }, [folder, refreshStatus, say]);

    const openDiff = useCallback(async (v) => {
      try {
        setDiff({ v, lines: await api("get_diff", { versionId: v.id }) });
      } catch (e) { say(String(e)); }
    }, [say]);

    const restore = useCallback(async (v) => {
      try {
        await api("restore_version", { versionId: v.id });
        setDiff(null);
        setConfirmId(null);
        await refreshStatus();
        say("Restored v" + v.number + " — previous state saved, undo anytime");
      } catch (e) { say(String(e)); }
    }, [refreshStatus, say]);

    const askRestore = useCallback((v) => {
      if (confirmId === v.id) {
        if (confirmTimer.current) clearTimeout(confirmTimer.current);
        restore(v);
        return;
      }
      setConfirmId(v.id);
      if (confirmTimer.current) clearTimeout(confirmTimer.current);
      confirmTimer.current = setTimeout(() => setConfirmId((c) => (c === v.id ? null : c)), 3000);
    }, [confirmId, restore]);

    useEffect(() => () => { if (confirmTimer.current) clearTimeout(confirmTimer.current); }, []);

    const reveal = useCallback(async (p) => {
      try { await api("reveal_path", { path: p }); }
      catch (e) { say(String(e)); }
    }, [say]);

    const copy = useCallback(async (text, label) => {
      const ok = await copyText(text);
      say(ok ? (label || "Copied") + ": " + trim(text, 60) : "Copy failed");
    }, [say]);

    /* --- ui effects ------------------------------------------------------- */
    useEffect(() => { setQ(""); }, [folder]);

    useEffect(() => {
      const onKey = (e) => {
        if (e.key === "Escape") { setDiff(null); return; }
        // Cmd/Ctrl+K focuses the file filter when a folder is open.
        if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === "k" && folder) {
          e.preventDefault();
          if (searchRef.current) searchRef.current.focus();
        }
      };
      window.addEventListener("keydown", onKey);
      return () => window.removeEventListener("keydown", onKey);
    }, [folder]);

    const submit = () => {
      if (prompt.trim()) watchFolder(prompt);
      else browse();
    };

    const watchInfo = status.watches.find((w) => w.path === folder);
    const selectedFile = fileList.find((f) => f.path === file);
    const shown = useMemo(() => {
      const needle = q.trim().toLowerCase();
      if (!needle) return fileList;
      return fileList.filter((f) => f.name.toLowerCase().includes(needle));
    }, [fileList, q]);

    /* --- render ----------------------------------------------------------- */
    return h("div", { className: "flex h-screen w-screen overflow-hidden bg-ink-900" },

      /* ============================== SIDEBAR ============================== */
      h("aside", { className: "w-[260px] h-full bg-ink-950 border-r border-ink-700 flex flex-col flex-shrink-0" },
        h("div", { className: "p-3 space-y-3 flex-1 min-h-0 flex flex-col" },

          // brand
          h("div", { className: "flex items-center justify-between px-1.5 pt-1" },
            h("button", {
              type: "button",
              className: "flex items-center gap-2.5 cursor-pointer bg-transparent border-none p-0 text-inherit",
              onClick: () => { setFolder(null); setFile(null); },
              "aria-label": "Home",
            },
              h(Logo, { c: "w-8 h-8" }),
              h("span", { className: "font-semibold text-sm tracking-tight" }, "Rewind")),
            h("span", {
              className: "text-[10px] font-mono text-zinc-500 px-1.5 py-0.5 rounded bg-ink-800 border border-ink-700",
              title: "Rewind version",
            }, "v0.5.0")),

          // primary CTA
          h("button", {
            type: "button", onClick: browse,
            className: "w-full py-2.5 px-3 rounded-lg bg-ink-800 hover:bg-ink-700 border border-ink-700 text-sm font-semibold flex items-center gap-2.5 transition",
          },
            h(Ic, { n: "plus", c: "w-4 h-4" }),
            h("span", null, "Watch folder")),

          // home
          h("button", {
            type: "button",
            onClick: () => { setFolder(null); setFile(null); },
            className: "w-full py-2 px-3 rounded-lg text-sm flex items-center gap-2.5 transition " +
              (!folder ? "text-zinc-100 bg-ink-850" : "text-zinc-400 hover:text-zinc-200 hover:bg-ink-850"),
          },
            h(Ic, { n: "home", c: "w-4 h-4" }),
            h("span", null, "Home")),

          // watches
          h("div", { className: "px-2 pt-3 flex items-center justify-between text-[11px] font-medium text-zinc-500 uppercase tracking-wider" },
            h("span", null, "Watched folders"),
            h("span", { className: "font-mono text-[10px]" }, status.watches.length)),

          h("div", { className: "flex-1 min-h-0 overflow-y-auto space-y-0.5 -mx-1 px-1" },
            status.watches.length === 0 &&
              h("div", { className: "px-2.5 py-2 text-xs text-zinc-600 leading-relaxed" },
                "Nothing yet. Add a folder and every change gets a snapshot."),

            status.watches.map((w) => {
              const sel = folder === w.path;
              return h("div", {
                key: w.path,
                onClick: () => { setFolder(w.path); setFile(null); },
                className: "group px-2.5 py-2 rounded-lg flex items-center justify-between text-sm cursor-pointer transition border " +
                  (sel ? "bg-ink-800 text-zinc-100 border-ink-700"
                       : "text-zinc-400 hover:text-zinc-200 hover:bg-ink-850 border-transparent"),
                title: w.path,
              },
                h("div", { className: "flex items-center gap-2.5 min-w-0 pr-2" },
                  h(Ic, { n: "folder", c: "w-4 h-4 flex-shrink-0 " + (sel ? "text-violet-400" : "text-zinc-500") }),
                  h("span", { className: "truncate" }, baseName(w.path))),
                h("div", { className: "flex items-center gap-1.5 flex-shrink-0" },
                  h("span", { className: "text-[11px] font-mono text-zinc-500 group-hover:hidden" }, w.versions),
                  h("button", {
                    type: "button",
                    onClick: (e) => { e.stopPropagation(); unwatch(w.path); },
                    className: "hidden group-hover:block text-zinc-500 hover:text-rose-400 focus-visible:block",
                    title: "Stop watching",
                    "aria-label": "Stop watching " + baseName(w.path),
                  }, h(Ic, { n: "x", c: "w-3.5 h-3.5" }))));
            }))),

        // footer: daemon
        h("div", { className: "p-3 border-t border-ink-700" },
          h("div", { className: "flex items-center gap-2.5 px-1" },
            h("span", { className: "w-2 h-2 rounded-full bg-emerald-400 rw-pulse flex-shrink-0" }),
            h("div", { className: "leading-tight min-w-0" },
              h("div", { className: "text-xs font-semibold text-zinc-300" }, "Local daemon"),
              h("div", { className: "text-[11px] text-zinc-500 font-mono truncate", title: status.data_dir },
                status.total_versions, " versions · ", status.total_files, " files"))))),

      /* ================================ MAIN ============================== */
      h("main", { className: "flex-1 h-full flex flex-col min-w-0 relative" },

        !folder
          /* ------------------------------ HOME ------------------------------ */
          ? h("div", { className: "flex-1 flex flex-col relative overflow-hidden" },
              h("div", { className: "absolute inset-0 rw-grid pointer-events-none" }),
              h("div", { className: "flex-1 flex flex-col items-center justify-center relative z-10 px-6" },
                h(Mascot, null),
                h("h1", { className: "mt-2 text-2xl font-semibold tracking-tight text-zinc-100 text-center" },
                  "Welcome, what do you want me to watch?"),
                h("p", { className: "mt-2 text-sm text-zinc-500 text-center max-w-md leading-relaxed" },
                  "Point me at a folder. I snapshot every change, so you can scrub back through time and restore any file.")),

              h("div", { className: "relative z-10 w-full max-w-3xl mx-auto pb-8 px-6" },
                h("div", { className: "flex items-center justify-between mb-2.5" },
                  h("button", { type: "button", onClick: browse, className: "chip" },
                    h(Ic, { n: "folder", c: "w-3.5 h-3.5" }), " Browse folder"),
                  h("span", { className: "chip text-zinc-500 font-mono" }, "local · no cloud")),

                h("div", { className: "rounded-2xl border border-ink-700 bg-ink-850 focus-within:border-violet-500/60 transition shadow-xl" },
                  h("textarea", {
                    rows: 2,
                    value: prompt,
                    onChange: (e) => setPrompt(e.target.value),
                    onKeyDown: (e) => {
                      if (e.key === "Enter" && !e.shiftKey) { e.preventDefault(); submit(); }
                    },
                    placeholder: "Paste a folder path like C:\\Users\\you\\projects\\my-app, or hit Browse…",
                    "aria-label": "Folder path",
                    className: "w-full bg-transparent border-none text-sm text-zinc-200 placeholder-zinc-500 focus:outline-none resize-none px-4 pt-3.5 font-sans",
                  }),
                  h("div", { className: "flex items-center justify-between border-t border-ink-700 px-3 py-2" },
                    h("div", { className: "flex items-center gap-3 text-xs text-zinc-500 font-mono" },
                      h("span", null, "blake3"),
                      h("span", { className: "text-zinc-700" }, "|"),
                      h("span", null, "SQLite"),
                      h("span", { className: "text-zinc-700" }, "|"),
                      h("span", { className: "flex items-center gap-1.5" },
                        h("span", { className: "w-1.5 h-1.5 rounded-full bg-emerald-400 rw-pulse" }),
                        " daemon on")),
                    h("button", {
                      type: "button",
                      onClick: submit,
                      disabled: busy,
                      "aria-label": prompt.trim() ? "Start watching" : "Browse for a folder",
                      className: "w-8 h-8 rounded-lg bg-violet-600 hover:bg-violet-500 disabled:opacity-50 text-white flex items-center justify-center transition",
                    }, h(Ic, { n: "up", c: "w-4 h-4" })))))))

          /* ----------------------------- BROWSER ---------------------------- */
          : h("div", { className: "flex-1 flex flex-col min-h-0" },
              h("header", { className: "h-14 border-b border-ink-700 px-5 flex items-center justify-between flex-shrink-0" },
                h("div", { className: "flex items-center gap-3 min-w-0" },
                  h(IconBtn, {
                    n: "left",
                    title: "Home",
                    onClick: () => { setFolder(null); setFile(null); },
                    cls: "p-1.5 rounded-lg bg-ink-800 border border-ink-700 text-zinc-400 hover:text-zinc-100 transition",
                  }),
                  h("div", { className: "min-w-0" },
                    h("div", { className: "text-sm font-semibold truncate" }, baseName(folder)),
                    h("div", { className: "text-[11px] font-mono text-zinc-500 truncate", title: folder }, folder))),

                h("div", { className: "flex items-center gap-3 flex-shrink-0" },
                  selectedFile && h("button", {
                    type: "button",
                    onClick: () => copy('rewind log "' + selectedFile.path + '"', "CLI"),
                    className: "chip font-mono",
                    title: "Copy the CLI log command",
                  }, h(Ic, { n: "copy", c: "w-3 h-3" }), " rewind log"),
                  h(Pill, { tone: "emerald" },
                    h("span", { className: "w-1.5 h-1.5 rounded-full bg-emerald-400 rw-pulse" }),
                    "Watching",
                    watchInfo && Number.isFinite(watchInfo.files) ? " · " + watchInfo.files + " files" : ""))),

              h("div", { className: "flex-1 flex min-h-0" },

                /* file rail */
                h("div", { className: "w-[290px] border-r border-ink-700 overflow-y-auto flex-shrink-0 p-3 space-y-1" },
                  h("div", { className: "px-2 py-1 text-[11px] font-medium text-zinc-500 uppercase tracking-wider flex items-center justify-between" },
                    h("span", null, "Files with history"),
                    h("span", { className: "font-mono text-[10px]" }, fileList.length)),

                  h("div", { className: "relative mb-1" },
                    h("span", { className: "absolute left-2.5 top-1/2 -translate-y-1/2 text-zinc-600 pointer-events-none" },
                      h(Ic, { n: "search", c: "w-3.5 h-3.5" })),
                    h("input", {
                      ref: searchRef,
                      value: q,
                      onChange: (e) => setQ(e.target.value),
                      placeholder: "Filter files…  (⌘K)",
                      "aria-label": "Filter files",
                      className: "w-full pl-7 pr-2.5 py-1.5 rounded-lg bg-ink-850 border border-ink-700 focus:border-violet-500/60 focus:outline-none text-xs text-zinc-200 placeholder-zinc-600 font-mono",
                    })),

                  fileList.length > 0 && shown.length === 0 &&
                    h("div", { className: "px-2 py-3 text-xs text-zinc-600" }, 'No file matches "', q, '".'),
                  fileList.length === 0 &&
                    h("div", { className: "px-2 py-3 text-xs text-zinc-600 leading-relaxed" },
                      filesState.loading
                        ? "Loading files…"
                        : "Snapshotting existing files… edit anything in this folder and it shows up here."),

                  shown.map((f) =>
                    h("div", {
                      key: f.path,
                      onClick: () => setFile(f.path),
                      className: "px-2.5 py-2 rounded-lg cursor-pointer border transition " +
                        (file === f.path ? "bg-ink-800 border-ink-700" : "border-transparent hover:bg-ink-850"),
                      title: f.path,
                    },
                      h("div", { className: "flex items-center gap-2 min-w-0" },
                        h(Ic, { n: "file", c: "w-3.5 h-3.5 flex-shrink-0 " + (file === f.path ? "text-violet-400" : "text-zinc-500") }),
                        h("span", { className: "text-xs font-mono truncate" }, f.name)),
                      h("div", { className: "mt-1 pl-[22px] flex items-center justify-between text-[11px] text-zinc-500" },
                        h("span", null, f.versions, " version", f.versions === 1 ? "" : "s"),
                        h("span", { className: "font-mono" }, relTime(f.last_change)))))),

                /* timeline */
                h("div", { className: "flex-1 overflow-y-auto p-6" },
                  !file
                    ? h("div", { className: "h-full flex items-center justify-center text-sm text-zinc-600" },
                        fileList.length === 0 && filesState.loading
                          ? "Loading…"
                          : "Select a file to see its timeline.")
                    : h("div", { className: "max-w-3xl mx-auto" },
                        h("div", { className: "mb-5 flex items-end justify-between gap-4" },
                          h("div", { className: "min-w-0" },
                            h("div", { className: "text-[11px] font-mono text-zinc-500 truncate" }, file),
                            h("h2", { className: "text-base font-semibold mt-0.5" },
                              "File history timeline ",
                              h("span", { className: "text-zinc-500 font-normal text-sm" },
                                "· ", hist.length, " version", hist.length === 1 ? "" : "s"))),
                          h("button", {
                            type: "button",
                            onClick: () => reveal(file),
                            className: "chip flex-shrink-0",
                          }, h(Ic, { n: "folder", c: "w-3.5 h-3.5" }), " Show in Explorer")),

                        h("div", { className: "relative pl-7 space-y-5 before:absolute before:left-[7px] before:top-3 before:bottom-3 before:w-[2px] before:bg-ink-700" },
                          hist.map((v) =>
                            h("div", { key: v.id, className: "relative group rw-fade" },
                              h("div", {
                                className: "absolute -left-7 top-5 w-4 h-4 rounded-full border-2 transition " +
                                  (v.is_current
                                    ? "bg-emerald-400 border-ink-900 ring-4 ring-emerald-500/20"
                                    : "bg-ink-800 border-ink-600 group-hover:border-violet-500"),
                              }),
                              h("div", {
                                className: "p-4 rounded-xl border transition " +
                                  (v.is_current ? "bg-ink-800 border-ink-600 shadow-lg"
                                                : "bg-ink-850 border-ink-700 hover:border-ink-600"),
                              },
                                h("div", { className: "flex items-center justify-between border-b border-ink-700 pb-3 mb-3 gap-3 flex-wrap" },
                                  h("div", { className: "flex items-center gap-3" },
                                    h("span", {
                                      className: "px-2 py-0.5 rounded text-xs font-mono font-bold border " +
                                        (v.is_current
                                          ? "bg-emerald-500/15 text-emerald-300 border-emerald-500/40"
                                          : "bg-ink-700 text-zinc-300 border-ink-600"),
                                    }, "v", v.number),
                                    h("span", { className: "text-xs text-zinc-400 font-mono" }, absTime(v.created_at)),
                                    h("span", { className: "text-[11px] text-zinc-500 font-mono" },
                                      "(", relTime(v.created_at), ")")),
                                  h("div", { className: "flex items-center gap-3 text-xs font-mono" },
                                    h("span", { className: "text-emerald-400" }, "+", v.lines_added),
                                    h("span", { className: "text-rose-400" }, "-", v.lines_deleted),
                                    h("span", { className: "text-zinc-600" }, "|"),
                                    h("button", {
                                      type: "button",
                                      title: "Click to copy full hash",
                                      onClick: () => copy(v.hash, "Hash"),
                                      className: "text-zinc-500 cursor-pointer hover:text-violet-300 bg-transparent border-none p-0 font-mono text-xs",
                                    }, (v.hash || "").slice(0, 8)))),

                                h("div", { className: "p-3 rounded-lg bg-ink-900 border border-ink-700 text-xs text-zinc-300 leading-relaxed flex items-center justify-between gap-3" },
                                  h("span", null, v.summary),
                                  h("span", { className: "text-zinc-500 font-mono flex-shrink-0" }, fmtSize(v.size))),

                                h("div", { className: "mt-3 flex items-center justify-end gap-2" },
                                  h("button", {
                                    type: "button",
                                    onClick: () => openDiff(v),
                                    className: "px-3 py-1.5 rounded-lg border border-ink-700 hover:border-ink-600 bg-ink-900 text-zinc-300 hover:text-white text-xs font-medium transition flex items-center gap-1.5",
                                  }, h(Ic, { n: "diff", c: "w-3.5 h-3.5 text-violet-400" }), " View diff"),
                                  !v.is_current && h("button", {
                                    type: "button",
                                    onClick: () => askRestore(v),
                                    className: "px-3 py-1.5 rounded-lg border text-xs font-medium transition flex items-center gap-1.5 " +
                                      (confirmId === v.id
                                        ? "border-violet-500 bg-violet-600 text-white"
                                        : "border-ink-700 hover:border-violet-500/50 bg-ink-900 hover:bg-violet-600/10 text-zinc-300 hover:text-violet-300"),
                                  }, h(Ic, { n: "restore", c: "w-3.5 h-3.5" }),
                                    " ", confirmId === v.id ? "Click again to confirm" : "Restore"))))))))),

        /* ----------------------------- DIFF MODAL -------------------------- */
        diff && h("div", {
          className: "fixed inset-0 bg-black/70 backdrop-blur-sm z-50 flex items-center justify-center p-6 rw-fade",
          onClick: () => setDiff(null),
          role: "dialog", "aria-modal": "true", "aria-label": "Version diff",
        },
          h("div", {
            className: "bg-ink-850 border border-ink-700 rounded-xl w-full max-w-4xl shadow-2xl flex flex-col overflow-hidden max-h-[85vh]",
            onClick: (e) => e.stopPropagation(),
          },
            h("div", { className: "p-4 border-b border-ink-700 flex items-center justify-between bg-ink-950 flex-shrink-0" },
              h("div", { className: "flex items-center gap-3 min-w-0" },
                h("div", { className: "px-2 py-0.5 rounded bg-violet-600/20 text-violet-300 font-mono text-xs border border-violet-500/30 flex-shrink-0" },
                  "v", diff.v.number, diff.v.number > 1 ? " vs previous" : " (initial)"),
                h("span", { className: "text-xs font-mono text-zinc-400 truncate" }, baseName(diff.v.path))),
              h(IconBtn, {
                n: "x", title: "Close", cls: "p-1 rounded text-zinc-400 hover:text-white",
                onClick: () => setDiff(null),
              })),

            /* min-h-0 is required so this flex child can actually scroll. */
            h("div", { className: "p-3 overflow-auto flex-1 min-h-0 font-mono text-xs bg-ink-950" },
              diff.lines.map((l, i) => {
                const cls =
                  l.kind === "add" ? "text-emerald-300 bg-emerald-950/40" :
                  l.kind === "del" ? "text-rose-300 bg-rose-950/40" :
                  l.kind === "hunk" ? "text-zinc-600 italic" : "text-zinc-400";
                const gutter =
                  l.kind === "add" ? "+" : l.kind === "del" ? "-" :
                  l.kind === "hunk" ? "" : " ";
                return h("div", { key: i, className: "rw-diff-row " + cls },
                  h("span", { className: "rw-diff-num" }, i + 1),
                  h("span", { className: "rw-diff-code" }, gutter + " " + l.text));
              })),

            h("div", { className: "p-4 border-t border-ink-700 flex items-center justify-between gap-3 flex-shrink-0" },
              h("span", { className: "text-xs text-zinc-500 truncate" },
                "CLI: ",
                h("code", { className: "text-violet-300 font-mono" },
                  'rewind restore "', baseName(diff.v.path), '" ', diff.v.number)),
              !diff.v.is_current && h("button", {
                type: "button",
                onClick: () => restore(diff.v),
                className: "px-4 py-2 rounded-lg bg-violet-600 hover:bg-violet-500 text-white text-xs font-semibold shadow-md shadow-violet-600/20 flex-shrink-0",
              }, "Restore v", diff.v.number)))),

        /* ------------------------------- TOASTS ---------------------------- */
        toasts.length > 0 && h("div", {
          className: "fixed bottom-6 right-6 flex flex-col items-end gap-2 z-[60]",
          role: "status", "aria-live": "polite",
        },
          toasts.map((t) =>
            h("div", {
              key: t.id,
              className: "rw-toast bg-violet-600 text-white font-mono text-xs px-4 py-2.5 rounded-lg shadow-xl flex items-center gap-2 max-w-md",
            },
              h(Ic, { n: "check", c: "w-4 h-4 flex-shrink-0" }),
              h("span", { className: "truncate" }, t.msg)))));
  }

  ReactDOM.createRoot(document.getElementById("root")).render(h(App, null));
})();