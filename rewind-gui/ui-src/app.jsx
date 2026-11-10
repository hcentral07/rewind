    const { useState, useEffect, useRef, useCallback } = React;

    /* ------------------------------------------------------------------ */
    /* backend bridge (Tauri) with a tiny demo fallback for browser preview */
    /* ------------------------------------------------------------------ */
    const tauriInvoke = window.__TAURI__ && window.__TAURI__.core ? window.__TAURI__.core.invoke : null;

    const demoNow = Date.now();
    const iso = (minsAgo) => new Date(demoNow - minsAgo * 60000).toISOString();
    const DEMO_ROOT = 'C:\\projects\\demo';
    const demoVersions = [
      { id: 3, number: 3, path: DEMO_ROOT + '\\src\\main.rs', hash: '7f9a2c1d00aa', size: 2210, created_at: iso(2), lines_added: 14, lines_deleted: 3, summary: 'Modified: +14 -3 lines (2.2 KB)', is_current: true },
      { id: 2, number: 2, path: DEMO_ROOT + '\\src\\main.rs', hash: '3e4b9d0f11bb', size: 1900, created_at: iso(49), lines_added: 28, lines_deleted: 12, summary: 'Modified: +28 -12 lines (1.9 KB)', is_current: false },
      { id: 1, number: 1, path: DEMO_ROOT + '\\src\\main.rs', hash: '8a1c5e4a22cc', size: 1500, created_at: iso(180), lines_added: 45, lines_deleted: 0, summary: 'Created: 45 lines (1.5 KB)', is_current: false }
    ];
    async function demo(cmd, args) {
      switch (cmd) {
        case 'get_status':
          return { data_dir: 'C:\\Users\\you\\AppData\\Local\\Rewind', total_versions: 3, total_files: 1,
                   watches: [{ path: DEMO_ROOT, added_at: iso(200), versions: 3, files: 1 }] };
        case 'pick_folder': return DEMO_ROOT;
        case 'watch_folder': return DEMO_ROOT;
        case 'list_files': return [{ path: DEMO_ROOT + '\\src\\main.rs', name: 'src\\main.rs', versions: 3, last_change: iso(2) }];
        case 'get_history': return demoVersions;
        case 'get_diff': return [
          { kind: 'ctx', text: 'fn main() {' },
          { kind: 'del', text: '    let (tx, rx) = std::sync::mpsc::channel();' },
          { kind: 'add', text: '    let (tx, rx) = tokio::sync::mpsc::unbounded_channel();' },
          { kind: 'hunk', text: '... 12 unchanged lines ...' },
          { kind: 'ctx', text: '}' }];
        default: return null;
      }
    }
    const api = (cmd, args) => (tauriInvoke ? tauriInvoke(cmd, args) : demo(cmd, args));

    /* ------------------------------------------------------------------ */
    /* helpers                                                              */
    /* ------------------------------------------------------------------ */
    const baseName = (p) => (p || '').split(/[\\/]/).filter(Boolean).pop() || p;
    const fmtSize = (b) => (b < 1024 ? b + ' B' : b < 1048576 ? (b / 1024).toFixed(1) + ' KB' : (b / 1048576).toFixed(1) + ' MB');
    function relTime(isoStr) {
      const t = new Date(isoStr).getTime();
      if (isNaN(t)) return '';
      const s = Math.max(0, Math.floor((Date.now() - t) / 1000));
      if (s < 10) return 'just now';
      if (s < 60) return s + 's ago';
      const m = Math.floor(s / 60);
      if (m < 60) return m + 'm ago';
      const h = Math.floor(m / 60);
      if (h < 24) return h + 'h ago';
      return Math.floor(h / 24) + 'd ago';
    }
    function absTime(isoStr) {
      const d = new Date(isoStr);
      return isNaN(d) ? isoStr : d.toLocaleString([], { month: 'short', day: 'numeric', hour: '2-digit', minute: '2-digit', second: '2-digit' });
    }

    const ICONS = {
      plus: 'M5 12h14M12 5v14',
      x: 'M18 6 6 18M6 6l12 12',
      up: 'm5 12 7-7 7 7M12 19V5',
      left: 'm12 19-7-7 7-7M19 12H5',
      folder: 'M20 20a2 2 0 0 0 2-2V8a2 2 0 0 0-2-2h-7.9a2 2 0 0 1-1.69-.9L9.6 3.9A2 2 0 0 0 7.93 3H4a2 2 0 0 0-2 2v13a2 2 0 0 0 2 2Z',
      restore: 'M3 12a9 9 0 1 0 9-9 9.75 9.75 0 0 0-6.74 2.74L3 8M3 3v5h5',
      file: 'M15 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V7ZM14 2v4a2 2 0 0 0 2 2h4',
      diff: 'M18 21a3 3 0 1 0 0-6 3 3 0 0 0 0 6ZM6 9a3 3 0 1 0 0-6 3 3 0 0 0 0 6ZM13 6h3a2 2 0 0 1 2 2v7M11 18H8a2 2 0 0 1-2-2V9',
      history: 'M3 12a9 9 0 1 0 9-9 9.75 9.75 0 0 0-6.74 2.74L3 8M3 3v5h5M12 7v5l4 2',
      home: 'm3 9 9-7 9 7v11a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2zM9 22V12h6v10',
      check: 'M20 6 9 17l-5-5',
      copy: 'M8 8h12v12H8zM4 16V4h12'
    };
    function Ic({ n, c = 'w-4 h-4' }) {
      return (
        <svg className={c} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
          <path d={ICONS[n]} />
        </svg>
      );
    }

    // App logo: ui/logo.png (set it with set-logo.bat). Falls back to a built-in icon.
    function Logo({ c = 'w-8 h-8' }) {
      const [bad, setBad] = useState(false);
      if (!bad) return <img src="logo.png" alt="Rewind" className={c + ' rounded-lg object-cover flex-shrink-0'} onError={() => setBad(true)} />;
      return (
        <div className={c + ' rounded-lg bg-violet-600/20 border border-violet-500/30 flex items-center justify-center text-violet-300 flex-shrink-0'}>
          <Ic n="history" c="w-4 h-4" />
        </div>
      );
    }

    /* ------------------------------------------------------------------ */
    /* mascot: blue-purple robot that follows the mouse                     */
    /* ------------------------------------------------------------------ */
    function Mascot() {
      const box = useRef(null);
      useEffect(() => {
        const el = box.current;
        if (!el) return;
        const mover = el.querySelector('[data-m="mover"]');
        const head = el.querySelector('[data-m="head"]');
        const eyes = el.querySelector('[data-m="eyes"]');
        const ears = el.querySelector('[data-m="ears"]');
        let tx = 0, ty = 0, x = 0, y = 0, raf = 0;

        const onMove = (e) => {
          const r = el.getBoundingClientRect();
          const dx = e.clientX - (r.left + r.width / 2);
          const dy = e.clientY - (r.top + r.height / 2);
          tx = Math.max(-1, Math.min(1, dx / (window.innerWidth * 0.45)));
          ty = Math.max(-1, Math.min(1, dy / (window.innerHeight * 0.45)));
        };
        const onLeave = () => { tx = 0; ty = 0; };
        const tick = () => {
          x += (tx - x) * 0.1;
          y += (ty - y) * 0.1;
          mover.style.transform = `translate(${x * 22}px, ${y * 14}px)`;
          head.style.transform = `translate(${x * 7}px, ${y * 5}px) rotate(${x * 5}deg)`;
          eyes.style.transform = `translate(${x * 11}px, ${y * 9}px)`;
          ears.style.transform = `translate(${x * 3}px, ${y * 2}px)`;
          raf = requestAnimationFrame(tick);
        };
        window.addEventListener('mousemove', onMove);
        document.addEventListener('mouseleave', onLeave);
        raf = requestAnimationFrame(tick);
        return () => {
          window.removeEventListener('mousemove', onMove);
          document.removeEventListener('mouseleave', onLeave);
          cancelAnimationFrame(raf);
        };
      }, []);

      const BODY = '#1d1a38';
      return (
        <div ref={box} className="relative">
          <div data-m="mover">
            <svg viewBox="0 0 260 260" className="w-56 h-56 overflow-visible rw-float">
              <defs>
                <linearGradient id="rwStroke" x1="0" y1="0" x2="1" y2="1">
                  <stop offset="0" stopColor="#5b8cff" />
                  <stop offset="1" stopColor="#8b5cf6" />
                </linearGradient>
                <radialGradient id="rwGlow">
                  <stop offset="0" stopColor="#7c6cff" stopOpacity=".38" />
                  <stop offset="1" stopColor="#7c6cff" stopOpacity="0" />
                </radialGradient>
                <filter id="rwBlur" x="-80%" y="-80%" width="260%" height="260%">
                  <feGaussianBlur stdDeviation="6" />
                </filter>
              </defs>
              <circle cx="130" cy="140" r="125" fill="url(#rwGlow)" />
              <g data-m="head">
                <g data-m="ears">
                  <polygon points="52,112 22,140 52,168" fill={BODY} stroke="url(#rwStroke)" strokeWidth="9" strokeLinejoin="round" />
                  <polygon points="208,112 238,140 208,168" fill={BODY} stroke="url(#rwStroke)" strokeWidth="9" strokeLinejoin="round" />
                </g>
                <rect x="112" y="30" width="36" height="56" rx="18" fill={BODY} stroke="url(#rwStroke)" strokeWidth="9" />
                <rect x="48" y="62" width="164" height="152" rx="44" fill={BODY} stroke="url(#rwStroke)" strokeWidth="9" />
                <rect x="117" y="55" width="26" height="14" fill={BODY} />
                <g data-m="eyes">
                  <g className="rw-blink">
                    <g filter="url(#rwBlur)" opacity=".7">
                      <rect x="90" y="116" width="16" height="46" rx="8" fill="url(#rwStroke)" />
                      <rect x="154" y="116" width="16" height="46" rx="8" fill="url(#rwStroke)" />
                    </g>
                    <rect x="90" y="116" width="16" height="46" rx="8" fill="url(#rwStroke)" />
                    <rect x="154" y="116" width="16" height="46" rx="8" fill="url(#rwStroke)" />
                  </g>
                </g>
              </g>
            </svg>
          </div>
        </div>
      );
    }

    /* ------------------------------------------------------------------ */
    /* app                                                                  */
    /* ------------------------------------------------------------------ */
    function App() {
      const [status, setStatus] = useState({ data_dir: '', total_versions: 0, total_files: 0, watches: [] });
      const [folder, setFolder] = useState(null);
      const [files, setFiles] = useState([]);
      const [file, setFile] = useState(null);
      const [history, setHistory] = useState([]);
      const [diff, setDiff] = useState(null);
      const [prompt, setPrompt] = useState('');
      const [busy, setBusy] = useState(false);
      const [toast, setToast] = useState(null);
      const toastTimer = useRef(null);
      const [q, setQ] = useState('');
      const [confirmId, setConfirmId] = useState(null);

      const say = (m) => {
        setToast(m);
        clearTimeout(toastTimer.current);
        toastTimer.current = setTimeout(() => setToast(null), 2600);
      };

      const refreshStatus = useCallback(async () => {
        try { setStatus(await api('get_status')); } catch (e) { /* ignore */ }
      }, []);

      useEffect(() => {
        refreshStatus();
        const t = setInterval(refreshStatus, 2500);
        return () => clearInterval(t);
      }, []);

      useEffect(() => {
        if (!folder) { setFiles([]); return; }
        let alive = true;
        const load = async () => {
          try {
            const f = await api('list_files', { folder });
            if (!alive) return;
            setFiles(f);
            setFile((cur) => (cur && f.some((x) => x.path === cur) ? cur : (f[0] ? f[0].path : null)));
          } catch (e) { /* ignore */ }
        };
        load();
        const t = setInterval(load, 2000);
        return () => { alive = false; clearInterval(t); };
      }, [folder]);

      useEffect(() => {
        if (!file) { setHistory([]); return; }
        let alive = true;
        const load = async () => {
          try { const h = await api('get_history', { path: file }); if (alive) setHistory(h); } catch (e) { /* ignore */ }
        };
        load();
        const t = setInterval(load, 2000);
        return () => { alive = false; clearInterval(t); };
      }, [file]);

      const watchFolder = async (p) => {
        const clean = (p || '').trim().replace(/^"|"$/g, '');
        if (!clean) return;
        setBusy(true);
        try {
          const key = await api('watch_folder', { path: clean });
          setFolder(key);
          setFile(null);
          setPrompt('');
          await refreshStatus();
          say('Watching ' + baseName(key));
        } catch (e) { say(String(e)); }
        setBusy(false);
      };

      const browse = async () => {
        try {
          const p = await api('pick_folder');
          if (p) watchFolder(p);
        } catch (e) { say(String(e)); }
      };

      const unwatch = async (path) => {
        try {
          await api('unwatch_folder', { path });
          if (folder === path) { setFolder(null); setFile(null); }
          await refreshStatus();
          say('Stopped watching ' + baseName(path) + ' (history kept)');
        } catch (e) { say(String(e)); }
      };

      const openDiff = async (v) => {
        try { setDiff({ v, lines: await api('get_diff', { versionId: v.id }) }); } catch (e) { say(String(e)); }
      };

      const restore = async (v) => {
        try {
          await api('restore_version', { versionId: v.id });
          setDiff(null);
          say('Restored v' + v.number + ' - the previous state is saved, undo anytime');
        } catch (e) { say(String(e)); }
      };

      const askRestore = (v) => {
        if (confirmId === v.id) { setConfirmId(null); restore(v); return; }
        setConfirmId(v.id);
        setTimeout(() => setConfirmId((c) => (c === v.id ? null : c)), 3000);
      };

      const reveal = async (p) => {
        try { await api('reveal_path', { path: p }); } catch (e) { say(String(e)); }
      };

      useEffect(() => { setQ(''); }, [folder]);
      useEffect(() => {
        const onKey = (e) => { if (e.key === 'Escape') setDiff(null); };
        window.addEventListener('keydown', onKey);
        return () => window.removeEventListener('keydown', onKey);
      }, []);

      const submit = () => { if (prompt.trim()) watchFolder(prompt); else browse(); };

      const copy = (text) => {
        try { navigator.clipboard.writeText(text); say('Copied: ' + text); } catch (e) { /* ignore */ }
      };

      const watchInfo = status.watches.find((w) => w.path === folder);
      const selectedFile = files.find((f) => f.path === file);
      const shown = files.filter((f) => f.name.toLowerCase().includes(q.trim().toLowerCase()));

      return (
        <div className="flex h-screen w-screen overflow-hidden bg-ink-900">

          {/* ---------------- sidebar ---------------- */}
          <aside className="w-[260px] h-full bg-ink-950 border-r border-ink-700 flex flex-col flex-shrink-0">
            <div className="p-3 space-y-3 flex-1 min-h-0 flex flex-col">
              <div className="flex items-center justify-between px-1.5 pt-1">
                <div className="flex items-center gap-2.5 cursor-pointer" onClick={() => { setFolder(null); setFile(null); }}>
                  <Logo c="w-8 h-8" />
                  <span className="font-semibold text-sm tracking-tight">Rewind</span>
                </div>
                <span className="text-[10px] font-mono text-zinc-500 px-1.5 py-0.5 rounded bg-ink-800 border border-ink-700">v0.1</span>
              </div>

              <button onClick={browse}
                className="w-full py-2.5 px-3 rounded-lg bg-ink-800 hover:bg-ink-700 border border-ink-700 text-sm font-semibold flex items-center gap-2.5 transition">
                <Ic n="plus" c="w-4 h-4" /> <span>Watch folder</span>
              </button>

              <button onClick={() => { setFolder(null); setFile(null); }}
                className={`w-full py-2 px-3 rounded-lg text-sm flex items-center gap-2.5 transition ${!folder ? 'text-zinc-100 bg-ink-850' : 'text-zinc-400 hover:text-zinc-200 hover:bg-ink-850'}`}>
                <Ic n="home" c="w-4 h-4" /> <span>Home</span>
              </button>

              <div className="px-2 pt-3 flex items-center justify-between text-[11px] font-medium text-zinc-500 uppercase tracking-wider">
                <span>Watched folders</span>
                <span className="font-mono text-[10px]">{status.watches.length}</span>
              </div>

              <div className="flex-1 min-h-0 overflow-y-auto space-y-0.5 -mx-1 px-1">
                {status.watches.length === 0 && (
                  <div className="px-2.5 py-2 text-xs text-zinc-600 leading-relaxed">Nothing yet. Add a folder and every change gets a snapshot.</div>
                )}
                {status.watches.map((w) => {
                  const sel = folder === w.path;
                  return (
                    <div key={w.path} onClick={() => { setFolder(w.path); setFile(null); }}
                      className={`group px-2.5 py-2 rounded-lg flex items-center justify-between text-sm cursor-pointer transition ${sel ? 'bg-ink-800 text-zinc-100 border border-ink-700' : 'text-zinc-400 hover:text-zinc-200 hover:bg-ink-850 border border-transparent'}`}
                      title={w.path}>
                      <div className="flex items-center gap-2.5 min-w-0 pr-2">
                        <Ic n="folder" c={`w-4 h-4 flex-shrink-0 ${sel ? 'text-violet-400' : 'text-zinc-500'}`} />
                        <span className="truncate">{baseName(w.path)}</span>
                      </div>
                      <div className="flex items-center gap-1.5 flex-shrink-0">
                        <span className="text-[11px] font-mono text-zinc-500 group-hover:hidden">{w.versions}</span>
                        <button onClick={(e) => { e.stopPropagation(); unwatch(w.path); }}
                          className="hidden group-hover:block text-zinc-500 hover:text-rose-400" title="Stop watching">
                          <Ic n="x" c="w-3.5 h-3.5" />
                        </button>
                      </div>
                    </div>
                  );
                })}
              </div>
            </div>

            <div className="p-3 border-t border-ink-700">
              <div className="flex items-center gap-2.5 px-1">
                <span className="w-2 h-2 rounded-full bg-emerald-400 rw-pulse flex-shrink-0" />
                <div className="leading-tight min-w-0">
                  <div className="text-xs font-semibold text-zinc-300">Local daemon</div>
                  <div className="text-[11px] text-zinc-500 font-mono truncate" title={status.data_dir}>{status.total_versions} versions &middot; {status.total_files} files</div>
                </div>
              </div>
            </div>
          </aside>

          {/* ---------------- main ---------------- */}
          <main className="flex-1 h-full flex flex-col min-w-0 relative">
            {!folder ? (
              /* ------------ home ------------ */
              <div className="flex-1 flex flex-col relative overflow-hidden">
                <div className="absolute inset-0 rw-grid pointer-events-none" />
                <div className="flex-1 flex flex-col items-center justify-center relative z-10 px-6">
                  <Mascot />
                  <h1 className="mt-2 text-2xl font-semibold tracking-tight text-zinc-100 text-center">Welcome, what do you want me to watch?</h1>
                  <p className="mt-2 text-sm text-zinc-500 text-center max-w-md leading-relaxed">
                    Point me at a folder. I snapshot every change, so you can scrub back through time and restore any file.
                  </p>
                </div>

                <div className="relative z-10 w-full max-w-3xl mx-auto pb-8 px-6">
                  <div className="flex items-center justify-between mb-2.5">
                    <button onClick={browse} className="chip"><Ic n="folder" c="w-3.5 h-3.5" /> Browse folder</button>
                    <span className="chip text-zinc-500 font-mono">local &middot; no cloud</span>
                  </div>
                  <div className="rounded-2xl border border-ink-700 bg-ink-850 focus-within:border-violet-500/60 transition shadow-xl">
                    <textarea
                      rows="2"
                      value={prompt}
                      onChange={(e) => setPrompt(e.target.value)}
                      onKeyDown={(e) => { if (e.key === 'Enter' && !e.shiftKey) { e.preventDefault(); submit(); } }}
                      placeholder="Paste a folder path like C:\Users\you\projects\my-app, or hit Browse..."
                      className="w-full bg-transparent border-none text-sm text-zinc-200 placeholder-zinc-500 focus:outline-none resize-none px-4 pt-3.5 font-sans"
                    />
                    <div className="flex items-center justify-between border-t border-ink-700 px-3 py-2">
                      <div className="flex items-center gap-3 text-xs text-zinc-500 font-mono">
                        <span>blake3</span><span className="text-zinc-700">|</span><span>SQLite</span><span className="text-zinc-700">|</span>
                        <span className="flex items-center gap-1.5"><span className="w-1.5 h-1.5 rounded-full bg-emerald-400 rw-pulse" /> daemon on</span>
                      </div>
                      <button onClick={submit} disabled={busy}
                        className="w-8 h-8 rounded-lg bg-violet-600 hover:bg-violet-500 disabled:opacity-50 text-white flex items-center justify-center transition">
                        <Ic n="up" c="w-4 h-4" />
                      </button>
                    </div>
                  </div>
                </div>
              </div>
            ) : (
              /* ------------ folder view ------------ */
              <div className="flex-1 flex flex-col min-h-0">
                <header className="h-14 border-b border-ink-700 px-5 flex items-center justify-between flex-shrink-0">
                  <div className="flex items-center gap-3 min-w-0">
                    <button onClick={() => { setFolder(null); setFile(null); }}
                      className="p-1.5 rounded-lg bg-ink-800 border border-ink-700 text-zinc-400 hover:text-zinc-100 transition" title="Home">
                      <Ic n="left" c="w-4 h-4" />
                    </button>
                    <div className="min-w-0">
                      <div className="text-sm font-semibold truncate">{baseName(folder)}</div>
                      <div className="text-[11px] font-mono text-zinc-500 truncate" title={folder}>{folder}</div>
                    </div>
                  </div>
                  <div className="flex items-center gap-3 flex-shrink-0">
                    {selectedFile && (
                      <button onClick={() => copy('rewind log "' + selectedFile.path + '"')} className="chip font-mono">
                        <Ic n="copy" c="w-3 h-3" /> rewind log
                      </button>
                    )}
                    <span className="inline-flex items-center gap-1.5 px-2.5 py-1 rounded-full bg-emerald-500/10 border border-emerald-500/20 text-emerald-400 text-xs font-medium">
                      <span className="w-1.5 h-1.5 rounded-full bg-emerald-400 rw-pulse" />
                      Watching{watchInfo ? ' \u00b7 ' + watchInfo.files + ' files' : ''}
                    </span>
                  </div>
                </header>

                <div className="flex-1 flex min-h-0">
                  {/* files */}
                  <div className="w-[290px] border-r border-ink-700 overflow-y-auto flex-shrink-0 p-3 space-y-1">
                    <div className="px-2 py-1 text-[11px] font-medium text-zinc-500 uppercase tracking-wider flex items-center justify-between">
                      <span>Files with history</span><span className="font-mono text-[10px]">{files.length}</span>
                    </div>
                    <input value={q} onChange={(e) => setQ(e.target.value)} placeholder="Filter files..."
                      className="w-full mb-1 px-2.5 py-1.5 rounded-lg bg-ink-850 border border-ink-700 focus:border-violet-500/60 focus:outline-none text-xs text-zinc-200 placeholder-zinc-600 font-mono" />
                    {files.length > 0 && shown.length === 0 && (
                      <div className="px-2 py-3 text-xs text-zinc-600">No file matches "{q}".</div>
                    )}
                    {files.length === 0 && (
                      <div className="px-2 py-3 text-xs text-zinc-600 leading-relaxed">
                        Snapshotting existing files... edit anything in this folder and it shows up here.
                      </div>
                    )}
                    {shown.map((f) => (
                      <div key={f.path} onClick={() => setFile(f.path)}
                        className={`px-2.5 py-2 rounded-lg cursor-pointer border transition ${file === f.path ? 'bg-ink-800 border-ink-700' : 'border-transparent hover:bg-ink-850'}`}
                        title={f.path}>
                        <div className="flex items-center gap-2 min-w-0">
                          <Ic n="file" c={`w-3.5 h-3.5 flex-shrink-0 ${file === f.path ? 'text-violet-400' : 'text-zinc-500'}`} />
                          <span className="text-xs font-mono truncate">{f.name}</span>
                        </div>
                        <div className="mt-1 pl-[22px] flex items-center justify-between text-[11px] text-zinc-500">
                          <span>{f.versions} version{f.versions === 1 ? '' : 's'}</span>
                          <span className="font-mono">{relTime(f.last_change)}</span>
                        </div>
                      </div>
                    ))}
                  </div>

                  {/* timeline */}
                  <div className="flex-1 overflow-y-auto p-6">
                    {!file ? (
                      <div className="h-full flex items-center justify-center text-sm text-zinc-600">Select a file to see its timeline.</div>
                    ) : (
                      <div className="max-w-3xl mx-auto">
                        <div className="mb-5 flex items-end justify-between gap-4">
                          <div className="min-w-0">
                            <div className="text-[11px] font-mono text-zinc-500 truncate">{file}</div>
                            <h2 className="text-base font-semibold mt-0.5">File history timeline <span className="text-zinc-500 font-normal text-sm">&middot; {history.length} version{history.length === 1 ? '' : 's'}</span></h2>
                          </div>
                          <button onClick={() => reveal(file)} className="chip flex-shrink-0"><Ic n="folder" c="w-3.5 h-3.5" /> Show in Explorer</button>
                        </div>

                        <div className="relative pl-7 space-y-5 before:absolute before:left-[7px] before:top-3 before:bottom-3 before:w-[2px] before:bg-ink-700">
                          {history.map((v) => (
                            <div key={v.id} className="relative group rw-fade">
                              <div className={`absolute -left-7 top-5 w-4 h-4 rounded-full border-2 transition ${v.is_current ? 'bg-emerald-400 border-ink-900 ring-4 ring-emerald-500/20' : 'bg-ink-800 border-ink-600 group-hover:border-violet-500'}`} />
                              <div className={`p-4 rounded-xl border transition ${v.is_current ? 'bg-ink-800 border-ink-600 shadow-lg' : 'bg-ink-850 border-ink-700 hover:border-ink-600'}`}>
                                <div className="flex items-center justify-between border-b border-ink-700 pb-3 mb-3 gap-3 flex-wrap">
                                  <div className="flex items-center gap-3">
                                    <span className={`px-2 py-0.5 rounded text-xs font-mono font-bold border ${v.is_current ? 'bg-emerald-500/15 text-emerald-300 border-emerald-500/40' : 'bg-ink-700 text-zinc-300 border-ink-600'}`}>v{v.number}</span>
                                    <span className="text-xs text-zinc-400 font-mono">{absTime(v.created_at)}</span>
                                    <span className="text-[11px] text-zinc-500 font-mono">({relTime(v.created_at)})</span>
                                  </div>
                                  <div className="flex items-center gap-3 text-xs font-mono">
                                    <span className="text-emerald-400">+{v.lines_added}</span>
                                    <span className="text-rose-400">-{v.lines_deleted}</span>
                                    <span className="text-zinc-600">|</span>
                                    <span className="text-zinc-500 cursor-pointer hover:text-violet-300" title="Click to copy full hash" onClick={() => copy(v.hash)}>{v.hash.slice(0, 8)}</span>
                                  </div>
                                </div>

                                <div className="p-3 rounded-lg bg-ink-900 border border-ink-700 text-xs text-zinc-300 leading-relaxed flex items-center justify-between gap-3">
                                  <span>{v.summary}</span>
                                  <span className="text-zinc-500 font-mono flex-shrink-0">{fmtSize(v.size)}</span>
                                </div>

                                <div className="mt-3 flex items-center justify-end gap-2">
                                  <button onClick={() => openDiff(v)}
                                    className="px-3 py-1.5 rounded-lg border border-ink-700 hover:border-ink-600 bg-ink-900 text-zinc-300 hover:text-white text-xs font-medium transition flex items-center gap-1.5">
                                    <Ic n="diff" c="w-3.5 h-3.5 text-violet-400" /> View diff
                                  </button>
                                  {!v.is_current && (
                                    <button onClick={() => askRestore(v)}
                                      className={`px-3 py-1.5 rounded-lg border text-xs font-medium transition flex items-center gap-1.5 ${confirmId === v.id ? 'border-violet-500 bg-violet-600 text-white' : 'border-ink-700 hover:border-violet-500/50 bg-ink-900 hover:bg-violet-600/10 text-zinc-300 hover:text-violet-300'}`}>
                                      <Ic n="restore" c="w-3.5 h-3.5" /> {confirmId === v.id ? 'Click again to confirm' : 'Restore'}
                                    </button>
                                  )}
                                </div>
                              </div>
                            </div>
                          ))}
                        </div>
                      </div>
                    )}
                  </div>
                </div>
              </div>
            )}
          </main>

          {/* ---------------- diff modal ---------------- */}
          {diff && (
            <div className="fixed inset-0 bg-black/70 backdrop-blur-sm z-50 flex items-center justify-center p-6 rw-fade" onClick={() => setDiff(null)}>
              <div className="bg-ink-850 border border-ink-700 rounded-xl w-full max-w-4xl shadow-2xl flex flex-col overflow-hidden max-h-[85vh]" onClick={(e) => e.stopPropagation()}>
                <div className="p-4 border-b border-ink-700 flex items-center justify-between bg-ink-950">
                  <div className="flex items-center gap-3 min-w-0">
                    <div className="px-2 py-0.5 rounded bg-violet-600/20 text-violet-300 font-mono text-xs border border-violet-500/30 flex-shrink-0">v{diff.v.number} vs previous</div>
                    <span className="text-xs font-mono text-zinc-400 truncate">{baseName(diff.v.path)}</span>
                  </div>
                  <button onClick={() => setDiff(null)} className="p-1 rounded text-zinc-400 hover:text-white"><Ic n="x" c="w-5 h-5" /></button>
                </div>

                <div className="p-3 overflow-auto flex-1 font-mono text-xs bg-ink-950">
                  {diff.lines.map((l, i) => (
                    <div key={i} className={'whitespace-pre px-2 py-0.5 rounded ' + (
                      l.kind === 'add' ? 'text-emerald-300 bg-emerald-950/40' :
                      l.kind === 'del' ? 'text-rose-300 bg-rose-950/40' :
                      l.kind === 'hunk' ? 'text-zinc-600 italic' : 'text-zinc-400')}>
                      {(l.kind === 'add' ? '+ ' : l.kind === 'del' ? '- ' : l.kind === 'hunk' ? '' : '  ') + l.text}
                    </div>
                  ))}
                </div>

                <div className="p-4 border-t border-ink-700 flex items-center justify-between gap-3">
                  <span className="text-xs text-zinc-500 truncate">CLI: <code className="text-violet-300 font-mono">rewind restore "{baseName(diff.v.path)}" {diff.v.number}</code></span>
                  {!diff.v.is_current && (
                    <button onClick={() => restore(diff.v)}
                      className="px-4 py-2 rounded-lg bg-violet-600 hover:bg-violet-500 text-white text-xs font-semibold shadow-md shadow-violet-600/20 flex-shrink-0">
                      Restore v{diff.v.number}
                    </button>
                  )}
                </div>
              </div>
            </div>
          )}

          {/* ---------------- toast ---------------- */}
          {toast && (
            <div className="rw-toast fixed bottom-6 right-6 bg-violet-600 text-white font-mono text-xs px-4 py-2.5 rounded-lg shadow-xl flex items-center gap-2 z-[60] max-w-md">
              <Ic n="check" c="w-4 h-4 flex-shrink-0" />
              <span className="truncate">{toast}</span>
            </div>
          )}
        </div>
      );
    }

    ReactDOM.createRoot(document.getElementById('root')).render(<App />);
