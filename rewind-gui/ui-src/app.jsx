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
    case 'unwatch_folder': return true;
    case 'restore_version': return true;
    case 'reveal_path': return true;
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
  copy: 'M8 8h12v12H8zM4 16V4h12',
  chevron: 'm9 18 6-6-6-6',
  more: 'M5 12h.01M12 12h.01M19 12h.01',
  external: 'M15 3h6v6M10 14 21 3M18 13v6a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h6',
  clock: 'M12 6v6l4 2',
  search: 'm21 21-4.3-4.3'
};
function Ic({ n, c = 'w-4 h-4' }) {
  return (
    <svg className={c} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
      <path d={ICONS[n]} />
    </svg>
  );
}

function Logo({ c = 'w-8 h-8' }) {
  const [bad, setBad] = useState(false);
  if (!bad) return <img src="logo.png" alt="Rewind" className={c + ' rounded-md object-cover flex-shrink-0'} onError={() => setBad(true)} />;
  return (
    <div className={c + ' rounded-md bg-zinc-800 border border-zinc-700 flex items-center justify-center text-zinc-400 flex-shrink-0'}>
      <Ic n="history" c="w-4 h-4" />
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
  const [showAll, setShowAll] = useState(false);

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
      say('Stopped watching ' + baseName(path) + '. History retained.');
    } catch (e) { say(String(e)); }
  };

  const openDiff = async (v) => {
    try { setDiff({ v, lines: await api('get_diff', { versionId: v.id }) }); } catch (e) { say(String(e)); }
  };

  const restore = async (v) => {
    try {
      await api('restore_version', { versionId: v.id });
      setDiff(null);
      say('Restored v' + v.number + '. Previous state saved.');
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
    const onKey = (e) => {
      if (e.key === 'Escape') setDiff(null);
      if (e.key === '/' && !e.target.matches('input, textarea')) { e.preventDefault(); document.getElementById('file-filter')?.focus(); }
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, []);

  const submit = () => { if (prompt.trim()) watchFolder(prompt); else browse(); };

  const copy = (text) => {
    try { navigator.clipboard.writeText(text); say('Copied'); } catch (e) { /* ignore */ }
  };

  const watchInfo = status.watches.find((w) => w.path === folder);
  const selectedFile = files.find((f) => f.path === file);
  const shown = files.filter((f) => f.name.toLowerCase().includes(q.trim().toLowerCase()));
  const visibleHistory = showAll ? history : history.slice(0, 5);
  const hasMore = history.length > 5;

  return (
    <div className="flex h-screen w-screen overflow-hidden bg-[#0a0a0b] text-zinc-300 font-sans">

      {/* ---------------- sidebar ---------------- */}
      <aside className="w-[240px] h-full bg-[#0f0f11] border-r border-zinc-800 flex flex-col flex-shrink-0">
        <div className="p-3 space-y-4 flex-1 min-h-0 flex flex-col">
          <div className="flex items-center gap-2 px-1 pt-1 cursor-pointer" onClick={() => { setFolder(null); setFile(null); }}>
            <Logo c="w-7 h-7" />
            <span className="font-semibold text-sm text-zinc-200 tracking-tight">Rewind</span>
          </div>

          <button onClick={browse}
            className="w-full py-2 px-3 rounded-md bg-zinc-800 hover:bg-zinc-700 border border-zinc-700 text-sm font-medium flex items-center gap-2 transition text-zinc-200">
            <Ic n="plus" c="w-3.5 h-3.5" /> <span>Add folder</span>
          </button>

          <button onClick={() => { setFolder(null); setFile(null); }}
            className={`w-full py-2 px-3 rounded-md text-sm flex items-center gap-2 transition ${!folder ? 'text-zinc-100 bg-zinc-800/70' : 'text-zinc-400 hover:text-zinc-200 hover:bg-zinc-800/50'}`}>
            <Ic n="home" c="w-3.5 h-3.5" /> <span>Home</span>
          </button>

          <div className="px-1 pt-2 flex items-center justify-between text-[11px] font-medium text-zinc-600">
            <span>WATCHED FOLDERS</span>
            <span className="font-mono">{status.watches.length}</span>
          </div>

          <div className="flex-1 min-h-0 overflow-y-auto space-y-0.5 -mx-1 px-1">
            {status.watches.length === 0 && (
              <div className="px-2 py-2 text-xs text-zinc-600 leading-relaxed">No folders yet.</div>
            )}
            {status.watches.map((w) => {
              const sel = folder === w.path;
              return (
                <div key={w.path} onClick={() => { setFolder(w.path); setFile(null); }}
                  className={`group px-2 py-1.5 rounded-md flex items-center justify-between text-sm cursor-pointer transition ${sel ? 'bg-zinc-800 text-zinc-100' : 'text-zinc-400 hover:text-zinc-200 hover:bg-zinc-800/50'}`}
                  title={w.path}>
                  <div className="flex items-center gap-2 min-w-0 pr-2">
                    <Ic n="folder" c={`w-3.5 h-3.5 flex-shrink-0 ${sel ? 'text-zinc-300' : 'text-zinc-600'}`} />
                    <span className="truncate text-[13px]">{baseName(w.path)}</span>
                  </div>
                  <div className="flex items-center gap-1 flex-shrink-0">
                    <span className="text-[11px] font-mono text-zinc-600 group-hover:hidden">{w.versions}</span>
                    <button onClick={(e) => { e.stopPropagation(); unwatch(w.path); }}
                      className="hidden group-hover:flex text-zinc-500 hover:text-zinc-200" title="Stop watching">
                      <Ic n="x" c="w-3.5 h-3.5" />
                    </button>
                  </div>
                </div>
              );
            })}
          </div>
        </div>

        <div className="p-3 border-t border-zinc-800">
          <div className="flex items-center gap-2 px-1">
            <span className="w-1.5 h-1.5 rounded-full bg-emerald-500 flex-shrink-0" />
            <div className="leading-tight min-w-0">
              <div className="text-[11px] font-medium text-zinc-400">Daemon running</div>
              <div className="text-[10px] text-zinc-600 font-mono truncate" title={status.data_dir}>{status.total_versions} versions · {status.total_files} files</div>
            </div>
          </div>
        </div>

        {/* Protected / Paused status indicator */}
        {folder && (
          <div className="p-3 border-t border-zinc-800">
            <div className="flex items-center gap-2 px-1">
              <span className="w-1.5 h-1.5 rounded-full bg-emerald-500 flex-shrink-0" />
              <div className="leading-tight min-w-0">
                <div className="text-emerald-400 font-medium">Protected</div>
                <div className="text-[10px] text-zinc-600 font-mono truncate">
                  {status.total_files > 0 && ' · ' + status.total_files + ' files'}
                </div>
              </div>
            </div>
          </div>
        )}
      </aside>

      {/* ---------------- main ---------------- */}
      <main className="flex-1 h-full flex flex-col min-w-0 relative">
        {!folder ? (
          /* ------------ home ------------ */
          <div className="flex-1 flex flex-col">
            <div className="flex-1 flex flex-col items-center justify-center px-6">
              <div className="w-full max-w-md text-center">
                <div className="mx-auto mb-4 w-12 h-12 rounded-md bg-zinc-900 border border-zinc-800 flex items-center justify-center">
                  <Ic n="history" c="w-5 h-5 text-zinc-500" />
                </div>
                <h1 className="text-lg font-semibold text-zinc-100">No folders watched</h1>
                <p className="mt-1.5 text-sm text-zinc-500 leading-relaxed">
                  Add a folder to track changes. Every edit becomes a snapshot you can restore.
                </p>
                <div className="mt-6 flex items-center justify-center gap-2">
                  <button onClick={browse}
                    className="px-4 py-2 rounded-md bg-zinc-100 text-zinc-900 text-sm font-medium hover:bg-white transition">
                    Choose folder
                  </button>
                </div>
              </div>
            </div>

            <div className="w-full max-w-2xl mx-auto pb-8 px-6">
              <div className="rounded-md border border-zinc-800 bg-zinc-900 focus-within:border-zinc-600 transition">
                <input
                  value={prompt}
                  onChange={(e) => setPrompt(e.target.value)}
                  onKeyDown={(e) => { if (e.key === 'Enter') submit(); }}
                  placeholder="C:\path\to\folder"
                  className="w-full bg-transparent px-3 py-2.5 text-sm text-zinc-200 placeholder-zinc-600 focus:outline-none font-mono"
                />
              </div>
              <p className="mt-2 text-[11px] text-zinc-600 font-mono">
                Local only · no cloud · {status.data_dir ? status.data_dir : 'app data dir'}
              </p>
            </div>
          </div>
        ) : (
          /* ------------ folder view ------------ */
          <div className="flex-1 flex flex-col min-h-0">
            <header className="h-12 border-b border-zinc-800 px-4 flex items-center justify-between flex-shrink-0">
              <div className="flex items-center gap-2 min-w-0">
                <button onClick={() => { setFolder(null); setFile(null); }}
                  className="p-1.5 rounded text-zinc-500 hover:text-zinc-200 hover:bg-zinc-800 transition" title="Back">
                  <Ic n="left" c="w-3.5 h-3.5" />
                </button>
                <div className="min-w-0">
                  <div className="text-sm font-medium truncate text-zinc-200">{baseName(folder)}</div>
                  <div className="text-[11px] font-mono text-zinc-600 truncate" title={folder}>{folder}</div>
                </div>
              </div>
              <div className="flex items-center gap-2 flex-shrink-0">
                {selectedFile && (
                  <button onClick={() => copy('rewind log "' + selectedFile.path + '"')}
                    className="px-2 py-1 rounded border border-zinc-800 hover:border-zinc-700 text-zinc-400 hover:text-zinc-200 text-[11px] font-mono flex items-center gap-1.5 transition">
                    <Ic n="copy" c="w-3 h-3" /> copy log cmd
                  </button>
                )}
                <span className="inline-flex items-center gap-1.5 px-2 py-0.5 rounded-full bg-emerald-500/10 border border-emerald-500/20 text-emerald-400 text-[11px] font-medium">
                  <span className="w-1.5 h-1.5 rounded-full bg-emerald-400" />
                  Watching{watchInfo ? ' · ' + watchInfo.files + ' files' : ''}
                </span>
              </div>
            </header>

            <div className="flex-1 flex min-h-0">
              {/* files */}
              <div className="w-[260px] border-r border-zinc-800 overflow-y-auto flex-shrink-0 p-2 space-y-0.5">
                <div className="px-2 py-1 text-[11px] font-medium text-zinc-600 flex items-center justify-between">
                  <span>FILES</span><span className="font-mono">{files.length}</span>
                </div>
                <div className="relative">
                  <Ic n="search" c="w-3 h-3 absolute left-2 top-1/2 -translate-y-1/2 text-zinc-600 pointer-events-none" />
                  <input id="file-filter" value={q} onChange={(e) => setQ(e.target.value)} placeholder="Filter  /"
                    className="w-full mb-1 pl-7 pr-2 py-1 rounded bg-zinc-900 border border-zinc-800 focus:border-zinc-600 focus:outline-none text-xs text-zinc-200 placeholder-zinc-600 font-mono" />
                </div>
                {files.length > 0 && shown.length === 0 && (
                  <div className="px-2 py-3 text-xs text-zinc-600">No match for "{q}".</div>
                )}
                {files.length === 0 && (
                  <div className="px-2 py-3 text-xs text-zinc-600 leading-relaxed">
                    Scanning… files appear as they change.
                  </div>
                )}
                {shown.map((f) => (
                  <div key={f.path} onClick={() => setFile(f.path)}
                    className={`px-2 py-1.5 rounded cursor-pointer transition ${file === f.path ? 'bg-zinc-800' : 'hover:bg-zinc-800/50'}`}
                    title={f.path}>
                    <div className="flex items-center gap-2 min-w-0">
                      <Ic n="file" c={`w-3 h-3 flex-shrink-0 ${file === f.path ? 'text-zinc-300' : 'text-zinc-600'}`} />
                      <span className="text-xs font-mono truncate text-zinc-300">{f.name}</span>
                    </div>
                    <div className="mt-0.5 pl-5 flex items-center justify-between text-[10px] text-zinc-600">
                      <span>{f.versions}v</span>
                      <span className="font-mono">{relTime(f.last_change)}</span>
                    </div>
                  </div>
                ))}
              </div>

              {/* timeline */}
              <div className="flex-1 overflow-y-auto">
                {!file ? (
                  <div className="h-full flex items-center justify-center text-sm text-zinc-600">Select a file.</div>
                ) : (
                  <div className="max-w-2xl mx-auto p-6">
                    <div className="mb-4 flex items-end justify-between gap-4">
                      <div className="min-w-0">
                        <div className="text-[11px] font-mono text-zinc-600 truncate">{file}</div>
                        <h2 className="text-sm font-medium mt-0.5 text-zinc-200">
                          {history.length} version{history.length === 1 ? '' : 's'}
                        </h2>
                      </div>
                      <button onClick={() => reveal(file)}
                        className="px-2 py-1 rounded border border-zinc-800 hover:border-zinc-700 text-zinc-400 hover:text-zinc-200 text-[11px] flex items-center gap-1.5 transition flex-shrink-0">
                        <Ic n="folder" c="w-3 h-3" /> Reveal
                      </button>
                    </div>

                    <div className="border border-zinc-800 rounded-md overflow-hidden divide-y divide-zinc-800">
                      {visibleHistory.map((v) => (
                        <div key={v.id} className={`group px-3 py-2.5 hover:bg-zinc-900/50 transition ${v.is_current ? 'bg-zinc-900/30' : ''}`}>
                          <div className="flex items-center gap-3">
                            <span className={`w-1.5 h-1.5 rounded-full flex-shrink-0 ${v.is_current ? 'bg-emerald-500' : 'bg-zinc-700'}`} />
                            <span className="font-mono text-xs text-zinc-500 w-8 flex-shrink-0">v{v.number}</span>
                            <span className="text-xs text-zinc-400 flex-1 truncate">{v.summary}</span>
                            <span className="font-mono text-[11px] text-zinc-600 flex-shrink-0 hidden sm:block" title={absTime(v.created_at)}>{relTime(v.created_at)}</span>
                            <div className="flex gap-0.5 opacity-0 group-hover:opacity-100 transition flex-shrink-0">
                              <button onClick={() => openDiff(v)} title="View diff"
                                className="p-1 rounded text-zinc-500 hover:text-zinc-200 hover:bg-zinc-800">
                                <Ic n="diff" c="w-3.5 h-3.5" />
                              </button>
                              {!v.is_current && (
                                <button onClick={() => askRestore(v)} title={confirmId === v.id ? 'Click again to confirm' : 'Restore'}
                                  className={`p-1 rounded transition ${confirmId === v.id ? 'text-zinc-100 bg-zinc-700' : 'text-zinc-500 hover:text-zinc-200 hover:bg-zinc-800'}`}>
                                <Ic n="restore" c="w-3.5 h-3.5" />
                              </button>
                            </div>
                          </div>
                          <div className="mt-1 pl-[26px] flex items-center gap-3 text-[11px] font-mono text-zinc-600">
                            <span className="text-emerald-500/70">+{v.lines_added}</span>
                            <span className="text-rose-500/70">-{v.lines_deleted}</span>
                            <span className="text-zinc-700">·</span>
                            <span>{fmtSize(v.size)}</span>
                            <span className="text-zinc-700">·</span>
                            <span className="cursor-pointer hover:text-zinc-400" title="Copy full hash" onClick={() => copy(v.hash)}>{v.hash.slice(0, 8)}</span>
                            {v.is_current && <span className="text-emerald-500/70">· current</span>}
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
        <div className="fixed inset-0 bg-black/60 z-50 flex items-center justify-center p-6" onClick={() => setDiff(null)}>
          <div className="bg-[#0f0f11] border border-zinc-800 rounded-md w-full max-w-4xl shadow-2xl flex flex-col overflow-hidden max-h-[85vh]" onClick={(e) => e.stopPropagation()}>
            <div className="px-4 py-3 border-b border-zinc-800 flex items-center justify-between">
              <div className="flex items-center gap-3 min-w-0">
                <span className="px-1.5 py-0.5 rounded bg-zinc-800 text-zinc-300 font-mono text-[11px] border border-zinc-700 flex-shrink-0">v{diff.v.number}</span>
                <span className="text-xs font-mono text-zinc-400 truncate">{baseName(diff.v.path)}</span>
                <span className="text-[11px] text-zinc-600 font-mono flex-shrink-0">vs previous</span>
              </div>
              <div className="flex items-center gap-1">
                <button onClick={() => reveal(diff.v.path)} title="Reveal in file manager"
                  className="p-1 rounded text-zinc-500 hover:text-zinc-200 hover:bg-zinc-800">
                  <Ic n="external" c="w-4 h-4" />
                </button>
                <button onClick={() => setDiff(null)} className="p-1 rounded text-zinc-500 hover:text-zinc-200 hover:bg-zinc-800"><Ic n="x" c="w-4 h-4" /></button>
              </div>
            </div>

            <div className="p-2 overflow-auto flex-1 font-mono text-xs bg-[#0a0a0b]">
              {diff.lines.map((l, i) => (
                <div key={i} className={'whitespace-pre px-2 py-0.5 ' + (
                  l.kind === 'add' ? 'text-emerald-300 bg-emerald-500/5' :
                  l.kind === 'del' ? 'text-rose-300 bg-rose-500/5' :
                  l.kind === 'hunk' ? 'text-zinc-600 italic' : 'text-zinc-400')}>
                  {(l.kind === 'add' ? '+ ' : l.kind === 'del' ? '- ' : l.kind === 'hunk' ? '' : '  ') + l.text}
                </div>
              ))}
            </div>

            <div className="px-4 py-3 border-t border-zinc-800 flex items-center justify-between gap-3">
              <button onClick={() => copy(`rewind restore "${baseName(diff.v.path)}" ${diff.v.number}`)}
                className="text-[11px] text-zinc-500 hover:text-zinc-300 font-mono flex items-center gap-1.5 transition">
                <Ic n="copy" c="w-3 h-3" />
                <span className="truncate">rewind restore {diff.v.number}</span>
              </button>
              {!diff.v.is_current && (
                <button onClick={() => restore(diff.v)}
                  className="px-3 py-1.5 rounded bg-zinc-100 hover:bg-white text-zinc-900 text-xs font-medium flex-shrink-0 transition">
                  Restore v{diff.v.number}
                </button>
              )}
            </div>
          </div>
        </div>
      )}

      {/* ---------------- toast ---------------- */}
      {toast && (
        <div className="fixed bottom-5 right-5 bg-zinc-800 border border-zinc-700 text-zinc-200 text-xs px-3 py-2 rounded-md shadow-xl flex items-center gap-2 z-[60] max-w-md">
          <Ic n="check" c="w-3.5 h-3.5 flex-shrink-0 text-emerald-400" />
          <span className="truncate">{toast}</span>
        </div>
      )}
    </div>
  );
}

ReactDOM.createRoot(document.getElementById('root')).render(<App />);