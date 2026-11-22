// Rewind tools panel: deleted files, search, versions (pin / compare), folder rewind,
// labels, plugins and settings. Plain React.createElement, so it needs no build step.
(function () {
  'use strict';
  var T = window.__TAURI__;
  var invoke = T && T.core ? T.core.invoke : function () { return Promise.reject('not running inside Rewind'); };
  var R = window.React, h = R.createElement;
  var useState = R.useState, useEffect = R.useEffect;

  function fmtTime(iso) { try { return new Date(iso).toLocaleString(); } catch (e) { return iso; } }
  function fmtSize(b) {
    if (b < 1024) return b + ' B';
    if (b < 1048576) return (b / 1024).toFixed(1) + ' KB';
    if (b < 1073741824) return (b / 1048576).toFixed(1) + ' MB';
    return (b / 1073741824).toFixed(2) + ' GB';
  }
  function msg(e) { return typeof e === 'string' ? e : (e && e.message) || String(e); }

  function Btn(p) {
    return h('button', { className: 'rwx-btn' + (p.kind ? ' ' + p.kind : ''), onClick: p.onClick, disabled: p.disabled, title: p.title }, p.children);
  }

  function FolderSelect(p) {
    return h('select', { className: 'rwx-in', style: { width: 'auto', maxWidth: '100%' }, value: p.value, onChange: function (e) { p.onChange(e.target.value); } },
      p.allowAll ? h('option', { value: '' }, 'All watched folders') : null,
      p.watches.map(function (w) { return h('option', { key: w, value: w }, w); }));
  }

  function Diff(p) {
    return h('div', { className: 'rwx-diff' }, p.lines.map(function (l, i) {
      var sign = l.kind === 'add' ? '+ ' : l.kind === 'del' ? '- ' : '  ';
      return h('div', { key: i, className: l.kind }, sign + l.text);
    }));
  }

  // ---- Deleted files -------------------------------------------------------
  function DeletedTab(p) {
    var a = useState([]), items = a[0], setItems = a[1];
    var b = useState(''), folder = b[0], setFolder = b[1];
    function load() { invoke('list_deleted', { folder: folder || null }).then(setItems).catch(function (e) { p.toast(msg(e)); }); }
    useEffect(function () { load(); }, [folder, p.tick]);
    function restore(it) {
      invoke('restore_deleted', { path: it.path }).then(function () { p.toast('Brought back ' + it.name); load(); }).catch(function (e) { p.toast(msg(e)); });
    }
    return h('div', null,
      h('div', { className: 'rwx-bar' }, h(FolderSelect, { watches: p.watches, value: folder, onChange: setFolder, allowAll: true }),
        h(Btn, { onClick: load }, 'Refresh')),
      items.length === 0 ? h('div', { className: 'rwx-muted' }, 'Nothing has been deleted from your watched folders.') : null,
      items.map(function (it) {
        return h('div', { className: 'rwx-row', key: it.path },
          h('div', { className: 'grow' }, h('div', { className: 't' }, it.name),
            h('div', { className: 's' }, it.path), h('div', { className: 's' }, 'deleted ' + fmtTime(it.deleted_at) + ' \u00b7 ' + it.versions + ' versions \u00b7 ' + fmtSize(it.size))),
          h(Btn, { kind: 'go', onClick: function () { restore(it); } }, 'Bring back'),
          h(Btn, { onClick: function () { p.openVersions(it.path); } }, 'Versions'));
      }));
  }

  // ---- Search ----------------------------------------------------------------
  function SearchTab(p) {
    var a = useState(''), q = a[0], setQ = a[1];
    var b = useState(''), folder = b[0], setFolder = b[1];
    var c = useState(null), hits = c[0], setHits = c[1];
    var d = useState(false), busy = d[0], setBusy = d[1];
    function go() {
      if (!q.trim()) return;
      setBusy(true);
      invoke('search_history', { query: q, folder: folder || null, limit: 100 })
        .then(function (r) { setHits(r); setBusy(false); })
        .catch(function (e) { setBusy(false); p.toast(msg(e)); });
    }
    return h('div', null,
      h('div', { className: 'rwx-bar' },
        h('input', { className: 'rwx-in', style: { flex: 1 }, placeholder: 'Search every saved version, e.g. API_KEY', value: q,
          onChange: function (e) { setQ(e.target.value); }, onKeyDown: function (e) { if (e.key === 'Enter') go(); } }),
        h(Btn, { kind: 'go', onClick: go, disabled: busy }, busy ? 'Searching...' : 'Search')),
      h('div', { className: 'rwx-bar' }, h(FolderSelect, { watches: p.watches, value: folder, onChange: setFolder, allowAll: true })),
      hits && hits.length === 0 ? h('div', { className: 'rwx-muted' }, 'No version contains that text.') : null,
      (hits || []).map(function (x) {
        return h('div', { className: 'rwx-row', key: x.version_id },
          h('div', { className: 'grow' },
            h('div', { className: 't' }, x.snippet || '(blank line)'),
            h('div', { className: 's' }, x.path + ' \u00b7 line ' + x.line_no + ' \u00b7 ' + fmtTime(x.created_at))),
          x.in_latest ? null : h('span', { className: 'rwx-badge gone' }, 'removed since'),
          h(Btn, { onClick: function () { p.openVersions(x.path); } }, 'Open'));
      }));
  }

  // ---- Versions: pin, compare, restore ----------------------------------------
  function VersionsTab(p) {
    var a = useState(p.path || ''), path = a[0], setPath = a[1];
    var b = useState([]), list = b[0], setList = b[1];
    var c = useState([]), sel = c[0], setSel = c[1];
    var d = useState(null), diff = d[0], setDiff = d[1];
    var e2 = useState(null), confirm = e2[0], setConfirm = e2[1];
    function load(pth) {
      var target = (pth || path).trim();
      if (!target) return;
      setDiff(null); setSel([]);
      invoke('get_history', { path: target }).then(setList).catch(function (e) { p.toast(msg(e)); });
    }
    useEffect(function () { if (p.path) { setPath(p.path); load(p.path); } }, [p.path]);
    function toggleSel(v) {
      setSel(function (s) { return s.indexOf(v.id) >= 0 ? s.filter(function (x) { return x !== v.id; }) : s.concat([v.id]).slice(-2); });
    }
    function pin(v) { invoke('pin_version', { versionId: v.id, pinned: !v.pinned }).then(function () { load(); }).catch(function (e) { p.toast(msg(e)); }); }
    function restore(v) {
      if (confirm !== v.id) { setConfirm(v.id); return; }
      setConfirm(null);
      invoke('restore_version', { versionId: v.id }).then(function () { p.toast('Restored v' + v.number); load(); }).catch(function (e) { p.toast(msg(e)); });
    }
    function compare() {
      var chosen = list.filter(function (v) { return sel.indexOf(v.id) >= 0; }).sort(function (x, y) { return x.number - y.number; });
      if (chosen.length !== 2) return;
      invoke('diff_versions', { oldId: chosen[0].id, newId: chosen[1].id }).then(setDiff).catch(function (e) { p.toast(msg(e)); });
    }
    return h('div', null,
      h('div', { className: 'rwx-bar' },
        h('input', { className: 'rwx-in', style: { flex: 1 }, placeholder: 'Full path of a file', value: path,
          onChange: function (e) { setPath(e.target.value); }, onKeyDown: function (e) { if (e.key === 'Enter') load(); } }),
        h(Btn, { kind: 'go', onClick: function () { load(); } }, 'Load')),
      p.has('compare') && list.length > 1 ? h('div', { className: 'rwx-bar' },
        h(Btn, { onClick: compare, disabled: sel.length !== 2 }, 'Compare the 2 ticked versions'),
        h('span', { className: 'rwx-muted' }, 'Tick any two versions, not just neighbours.')) : null,
      diff ? h('div', { style: { marginBottom: 12 } }, h(Diff, { lines: diff })) : null,
      list.map(function (v) {
        return h('div', { className: 'rwx-row', key: v.id },
          p.has('compare') ? h('input', { type: 'checkbox', checked: sel.indexOf(v.id) >= 0, onChange: function () { toggleSel(v); } }) : null,
          h('div', { className: 'grow' },
            h('div', { className: 't' }, 'v' + v.number + (v.is_current ? '  (newest)' : '') + (v.pinned ? '  \u2605 pinned' : '')),
            h('div', { className: 's' }, fmtTime(v.created_at) + ' \u00b7 ' + v.summary)),
          p.has('pins-labels') ? h(Btn, { onClick: function () { pin(v); }, title: 'Pinned versions are never pruned' }, v.pinned ? 'Unpin' : 'Pin') : null,
          v.is_current ? null : h(Btn, { kind: confirm === v.id ? 'warn' : '', onClick: function () { restore(v); } }, confirm === v.id ? 'Click again to restore' : 'Restore'));
      }));
  }

  // ---- Folder rewind ------------------------------------------------------------
  function toLocalInput(d) {
    var pad = function (n) { return (n < 10 ? '0' : '') + n; };
    return d.getFullYear() + '-' + pad(d.getMonth() + 1) + '-' + pad(d.getDate()) + 'T' + pad(d.getHours()) + ':' + pad(d.getMinutes());
  }
  function RewindTab(p) {
    var a = useState(p.folder || p.watches[0] || ''), folder = a[0], setFolder = a[1];
    var b = useState(p.to ? toLocalInput(new Date(p.to)) : toLocalInput(new Date(Date.now() - 3600000))), when = b[0], setWhen = b[1];
    var c = useState(null), plan = c[0], setPlan = c[1];
    var d = useState(false), removeNew = d[0], setRemoveNew = d[1];
    var e2 = useState(false), confirm = e2[0], setConfirm = e2[1];
    function iso() { return new Date(when).toISOString(); }
    function preview() {
      setConfirm(false);
      invoke('plan_rewind', { folder: folder, to: iso() }).then(setPlan).catch(function (e) { setPlan(null); p.toast(msg(e)); });
    }
    function apply() {
      if (!confirm) { setConfirm(true); return; }
      setConfirm(false);
      invoke('apply_rewind', { folder: folder, to: iso(), removeNew: removeNew }).then(function (r) {
        p.toast('Restored ' + r.restored + ', brought back ' + r.recreated + ', removed ' + r.removed + (r.failed.length ? ', ' + r.failed.length + ' failed' : ''));
        setPlan(null);
      }).catch(function (e) { p.toast(msg(e)); });
    }
    var quick = [['1 hour ago', 1], ['3 hours ago', 3], ['Yesterday', 24], ['1 week ago', 168]];
    var news = plan ? plan.items.filter(function (i) { return i.action === 'remove'; }).length : 0;
    var changes = plan ? plan.items.filter(function (i) { return i.action !== 'remove' || removeNew; }).length : 0;
    return h('div', null,
      h('div', { className: 'rwx-bar' }, h(FolderSelect, { watches: p.watches, value: folder, onChange: setFolder })),
      h('div', { className: 'rwx-bar' },
        h('input', { type: 'datetime-local', className: 'rwx-in', style: { width: 'auto' }, value: when, onChange: function (e) { setWhen(e.target.value); setPlan(null); } }),
        quick.map(function (q) { return h(Btn, { key: q[0], onClick: function () { setWhen(toLocalInput(new Date(Date.now() - q[1] * 3600000))); setPlan(null); } }, q[0]); })),
      h('div', { className: 'rwx-bar' }, h(Btn, { kind: 'go', onClick: preview, disabled: !folder }, 'Preview what would change')),
      plan ? h('div', null,
        h('div', { className: 'rwx-muted', style: { marginBottom: 8 } }, plan.items.length + ' files differ, ' + plan.unchanged + ' already match. A label is saved first, so you can undo this.'),
        plan.items.map(function (i) {
          return h('div', { className: 'rwx-row', key: i.path },
            h('div', { className: 'grow' }, h('div', { className: 't' }, i.path), h('div', { className: 's' }, i.summary)),
            h('span', { className: 'rwx-badge' + (i.action === 'remove' ? ' gone' : '') }, i.action));
        }),
        news ? h('label', { className: 'rwx-sw', style: { margin: '8px 0' } }, h('span', null, 'Also remove ' + news + ' files that did not exist yet (kept in history)'),
          h('input', { type: 'checkbox', checked: removeNew, onChange: function (e) { setRemoveNew(e.target.checked); } })) : null,
        h(Btn, { kind: confirm ? 'warn' : 'go', onClick: apply, disabled: changes === 0 }, confirm ? 'Click again to rewind the folder' : 'Rewind folder')) : null);
  }

  // ---- Labels -------------------------------------------------------------------
  function LabelsTab(p) {
    var a = useState(p.watches[0] || ''), folder = a[0], setFolder = a[1];
    var b = useState(''), name = b[0], setName = b[1];
    var c = useState([]), labels = c[0], setLabels = c[1];
    function load() { invoke('list_labels', { folder: folder || null }).then(setLabels).catch(function (e) { p.toast(msg(e)); }); }
    useEffect(function () { load(); }, [folder]);
    function add() {
      if (!name.trim() || !folder) return;
      invoke('add_label', { folder: folder, name: name }).then(function () { setName(''); p.toast('Moment saved'); load(); }).catch(function (e) { p.toast(msg(e)); });
    }
    return h('div', null,
      h('div', { className: 'rwx-bar' }, h(FolderSelect, { watches: p.watches, value: folder, onChange: setFolder })),
      h('div', { className: 'rwx-bar' },
        h('input', { className: 'rwx-in', style: { flex: 1 }, placeholder: 'Name this moment, e.g. before mod install', value: name,
          onChange: function (e) { setName(e.target.value); }, onKeyDown: function (e) { if (e.key === 'Enter') add(); } }),
        h(Btn, { kind: 'go', onClick: add }, 'Save moment')),
      labels.length === 0 ? h('div', { className: 'rwx-muted' }, 'No labels yet. Saving a moment snapshots the folder right now.') : null,
      labels.map(function (l) {
        return h('div', { className: 'rwx-row', key: l.id },
          h('div', { className: 'grow' }, h('div', { className: 't' }, l.name), h('div', { className: 's' }, fmtTime(l.created_at) + ' \u00b7 ' + l.folder)),
          p.has('folder-rewind') ? h(Btn, { onClick: function () { p.openRewind(l.folder, l.created_at); } }, 'Rewind to this') : null,
          h(Btn, { onClick: function () { invoke('delete_label', { id: l.id }).then(load); } }, 'Delete'));
      }));
  }


  // ---- Folder history (opened by the right-click menu on a folder) ---------------
  function FolderTab(p) {
    var a = useState([]), files = a[0], setFiles = a[1];
    var b = useState([]), labels = b[0], setLabels = b[1];
    var c = useState([]), gone = c[0], setGone = c[1];
    var d = useState(''), name = d[0], setName = d[1];
    function load() {
      invoke('list_files', { folder: p.folder }).then(setFiles).catch(function (e) { p.toast(msg(e)); });
      invoke('list_labels', { folder: p.folder }).then(setLabels).catch(function () {});
      invoke('list_deleted', { folder: p.folder }).then(setGone).catch(function () {});
    }
    useEffect(load, [p.folder]);
    function save() {
      invoke('add_label', { folder: p.folder, name: name || 'Moment' }).then(function () { setName(''); p.toast('Moment saved'); load(); }).catch(function (e) { p.toast(msg(e)); });
    }
    return h('div', null,
      h('div', { className: 'rwx-h', style: { marginTop: 0 } }, p.folder),
      h('div', { className: 'rwx-bar' },
        p.has('folder-rewind') ? h(Btn, { kind: 'go', onClick: function () { p.openRewind(p.folder, ''); } }, 'Rewind this folder') : null,
        h('input', { className: 'rwx-in', style: { flex: 1 }, placeholder: 'Name this moment (optional)', value: name, onChange: function (e) { setName(e.target.value); } }),
        h(Btn, { onClick: save }, 'Save moment')),
      gone.length ? h('div', { className: 'rwx-muted', style: { marginBottom: 8 } }, gone.length + ' deleted files can be brought back from the Deleted files tab.') : null,
      labels.slice(0, 3).map(function (l) {
        return h('div', { className: 'rwx-row', key: 'l' + l.id }, h('div', { className: 'grow' }, h('div', { className: 't' }, l.name), h('div', { className: 's' }, fmtTime(l.created_at))),
          p.has('folder-rewind') ? h(Btn, { onClick: function () { p.openRewind(l.folder, l.created_at); } }, 'Rewind to this') : null);
      }),
      h('div', { className: 'rwx-h' }, 'Most recently changed'),
      files.length === 0 ? h('div', { className: 'rwx-muted' }, 'No history for this folder yet. Add it with the Watch button in the main window.') : null,
      files.slice(0, 60).map(function (f) {
        return h('div', { className: 'rwx-row', key: f.path },
          h('div', { className: 'grow' }, h('div', { className: 't' }, f.name), h('div', { className: 's' }, fmtTime(f.last_change) + ' \u00b7 ' + f.versions + ' versions')),
          h(Btn, { onClick: function () { p.openVersions(f.path); } }, 'Versions'));
      }));
  }

  // ---- Protection: tripwire + alerts ------------------------------------------------
  function GuardTab(p) {
    var a = useState([]), alerts = a[0], setAlerts = a[1];
    var b = useState({ files: 50, seconds: 10, suspicious: 5 }), cfg = b[0], setCfg = b[1];
    var c = useState(null), plan = c[0], setPlan = c[1];
    var d = useState(false), removeNew = d[0], setRemoveNew = d[1];
    var e2 = useState(false), confirm = e2[0], setConfirm = e2[1];
    function load() {
      invoke('list_alerts', { unseenOnly: false }).then(setAlerts).catch(function (e) { p.toast(msg(e)); });
      invoke('tripwire_get').then(setCfg).catch(function () {});
      p.refreshAlerts();
    }
    useEffect(load, [p.tick]);
    function preview(al) {
      setConfirm(false);
      invoke('plan_alert_rewind', { id: al.id }).then(function (pl) { setPlan({ alert: al, plan: pl }); }).catch(function (e) { p.toast(msg(e)); });
    }
    function undo() {
      if (!confirm) { setConfirm(true); return; }
      setConfirm(false);
      invoke('apply_alert_rewind', { id: plan.alert.id, removeNew: removeNew }).then(function (r) {
        p.toast('Put back ' + (r.restored + r.recreated) + ' files' + (r.removed ? ', removed ' + r.removed + ' new ones' : ''));
        setPlan(null); load();
      }).catch(function (e) { p.toast(msg(e)); });
    }
    function save() {
      invoke('tripwire_set', { files: parseInt(cfg.files, 10) || 50, seconds: parseInt(cfg.seconds, 10) || 10 }).then(function () { p.toast('Tripwire saved'); load(); }).catch(function (e) { p.toast(msg(e)); });
    }
    var tw = alerts.filter(function (x) { return x.kind === 'tripwire'; });
    var other = alerts.filter(function (x) { return x.kind !== 'tripwire'; });
    function row(al) {
      return h('div', { className: 'rwx-row', key: al.id, style: al.seen ? {} : { borderColor: '#7f2a31' } },
        h('div', { className: 'grow' }, h('div', { className: 't' }, al.title + (al.seen ? '' : '  \u2022 new')),
          h('div', { className: 's', style: { whiteSpace: 'normal' } }, al.message), h('div', { className: 's' }, fmtTime(al.created_at))),
        al.kind === 'tripwire' && al.window_start ? h(Btn, { kind: 'go', onClick: function () { preview(al); } }, 'Preview undo') : null,
        al.seen ? null : h(Btn, { onClick: function () { invoke('ack_alerts', { id: al.id }).then(load); } }, 'Dismiss'));
    }
    return h('div', null,
      h('div', { className: 'rwx-h', style: { marginTop: 0 } }, 'Tripwire'),
      h('div', { className: 'rwx-muted', style: { marginBottom: 8, whiteSpace: 'normal' } },
        'Rewind raises an alert when many files change, vanish or turn into random-looking bytes within seconds (ransomware, a runaway script, a bad bulk edit) and pins the last good version of each file so no prune can delete it.'),
      h('div', { className: 'rwx-bar' },
        h('span', null, 'Trip at'), h('input', { className: 'rwx-in', style: { width: 70 }, value: cfg.files, onChange: function (e) { setCfg(Object.assign({}, cfg, { files: e.target.value })); } }),
        h('span', null, 'files within'), h('input', { className: 'rwx-in', style: { width: 70 }, value: cfg.seconds, onChange: function (e) { setCfg(Object.assign({}, cfg, { seconds: e.target.value })); } }),
        h('span', null, 'seconds'), h(Btn, { onClick: save }, 'Save')),
      h('div', { className: 'rwx-muted' }, 'Also trips when ' + cfg.suspicious + ' files turn into random-looking bytes.'),
      plan ? h('div', { style: { margin: '12px 0' } },
        h('div', { className: 'rwx-h' }, 'Undo: ' + plan.plan.folder),
        h('div', { className: 'rwx-muted', style: { marginBottom: 8 } }, plan.plan.items.length + ' files would change, ' + plan.plan.unchanged + ' already match. A label is saved first, so this can be undone too.'),
        plan.plan.items.slice(0, 40).map(function (i) { return h('div', { className: 'rwx-row', key: i.path }, h('div', { className: 'grow' }, h('div', { className: 't' }, i.path)), h('span', { className: 'rwx-badge' + (i.action === 'remove' ? ' gone' : '') }, i.action)); }),
        h('label', { className: 'rwx-sw', style: { margin: '8px 0' } }, h('span', null, 'Also remove files that did not exist before the burst (kept in history)'), h('input', { type: 'checkbox', checked: removeNew, onChange: function (e) { setRemoveNew(e.target.checked); } })),
        h('div', { className: 'rwx-bar' }, h(Btn, { kind: confirm ? 'warn' : 'go', onClick: undo, disabled: plan.plan.items.length === 0 }, confirm ? 'Click again to put the folder back' : 'Put the folder back'), h(Btn, { onClick: function () { setPlan(null); } }, 'Cancel'))) : null,
      h('div', { className: 'rwx-h' }, 'Tripwire alerts'),
      tw.length === 0 ? h('div', { className: 'rwx-muted' }, 'No alerts. Nothing suspicious has happened.') : null,
      tw.map(row),
      other.length ? h('div', null, h('div', { className: 'rwx-h' }, 'Plugin and system alerts'), other.map(row), h(Btn, { onClick: function () { invoke('ack_alerts', { id: null }).then(load); } }, 'Dismiss all')) : null);
  }

  // ---- AI agents -----------------------------------------------------------------------
  function AgentsTab(p) {
    var a = useState([]), runs = a[0], setRuns = a[1];
    var b = useState(p.watches[0] || ''), folder = b[0], setFolder = b[1];
    var c = useState('my agent'), agent = c[0], setAgent = c[1];
    var d = useState(null), report = d[0], setReport = d[1];
    var e2 = useState(null), plan = e2[0], setPlan = e2[1];
    var f = useState(false), removeNew = f[0], setRemoveNew = f[1];
    var g = useState(false), confirm = g[0], setConfirm = g[1];
    function load() { invoke('agent_runs').then(setRuns).catch(function (e) { p.toast(msg(e)); }); }
    useEffect(load, [p.tick]);
    function start() {
      if (!folder) return;
      invoke('agent_start', { folder: folder, agent: agent }).then(function (r) { p.toast('Checkpoint saved. Run ' + r.id + ' started.'); load(); }).catch(function (e) { p.toast(msg(e)); });
    }
    function show(r) { invoke('agent_report', { id: r.id }).then(function (x) { setReport(x); setPlan(null); }).catch(function (e) { p.toast(msg(e)); }); }
    function end(r) { invoke('agent_end', { id: r.id }).then(function (x) { setReport(x); load(); }).catch(function (e) { p.toast(msg(e)); }); }
    function preview(r) { setConfirm(false); invoke('agent_undo_plan', { id: r.id }).then(function (pl) { setPlan({ run: r, plan: pl }); }).catch(function (e) { p.toast(msg(e)); }); }
    function undo() {
      if (!confirm) { setConfirm(true); return; }
      setConfirm(false);
      invoke('agent_undo', { id: plan.run.id, removeNew: removeNew }).then(function (r) {
        p.toast('Undone: restored ' + r.restored + ', brought back ' + r.recreated + ', removed ' + r.removed);
        setPlan(null); setReport(null); load();
      }).catch(function (e) { p.toast(msg(e)); });
    }
    return h('div', null,
      h('div', { className: 'rwx-muted', style: { marginBottom: 8, whiteSpace: 'normal' } },
        'Before an AI coding agent works on a project, save a checkpoint. Afterwards see exactly what it touched and undo the whole run in one click. From a terminal: rewind run -- claude (or any command). For agents that support MCP: rewind mcp.'),
      h('div', { className: 'rwx-bar' }, h(FolderSelect, { watches: p.watches, value: folder, onChange: setFolder }),
        h('input', { className: 'rwx-in', style: { width: 150 }, value: agent, onChange: function (e) { setAgent(e.target.value); }, placeholder: 'agent name' }),
        h(Btn, { kind: 'go', onClick: start, disabled: !folder }, 'Checkpoint and start')),
      report ? h('div', { style: { margin: '12px 0' } },
        h('div', { className: 'rwx-h' }, 'Run ' + report.run.id + ' (' + report.run.agent + ')'),
        h('div', { className: 'rwx-muted' }, report.created + ' created, ' + report.modified + ' modified, ' + report.deleted + ' deleted (+' + report.lines_added + ' -' + report.lines_deleted + ' lines)'),
        report.flags.map(function (fl, i) { return h('div', { key: 'f' + i, className: 'rwx-row', style: { borderColor: '#7f2a31' } }, '! ' + fl); }),
        report.changes.slice(0, 80).map(function (ch) {
          return h('div', { className: 'rwx-row', key: ch.path }, h('div', { className: 'grow' }, h('div', { className: 't' }, ch.path), h('div', { className: 's' }, '+' + ch.lines_added + ' -' + ch.lines_deleted)),
            h('span', { className: 'rwx-badge' + (ch.action === 'deleted' ? ' gone' : '') }, ch.action),
            ch.action === 'deleted' ? null : h(Btn, { onClick: function () { p.openVersions(ch.path); } }, 'Versions'));
        })) : null,
      plan ? h('div', { style: { margin: '12px 0' } },
        h('div', { className: 'rwx-h' }, 'Undo run ' + plan.run.id),
        h('div', { className: 'rwx-muted', style: { marginBottom: 8 } }, plan.plan.items.length + ' files would change. A label is saved first, so the undo can be undone.'),
        plan.plan.items.slice(0, 40).map(function (i) { return h('div', { className: 'rwx-row', key: i.path }, h('div', { className: 'grow' }, h('div', { className: 't' }, i.path)), h('span', { className: 'rwx-badge' + (i.action === 'remove' ? ' gone' : '') }, i.action)); }),
        h('label', { className: 'rwx-sw', style: { margin: '8px 0' } }, h('span', null, 'Also remove files the agent created (kept in history)'), h('input', { type: 'checkbox', checked: removeNew, onChange: function (e) { setRemoveNew(e.target.checked); } })),
        h('div', { className: 'rwx-bar' }, h(Btn, { kind: confirm ? 'warn' : 'go', onClick: undo, disabled: plan.plan.items.length === 0 }, confirm ? 'Click again to undo the run' : 'Undo the run'), h(Btn, { onClick: function () { setPlan(null); } }, 'Cancel'))) : null,
      h('div', { className: 'rwx-h' }, 'Runs'),
      runs.length === 0 ? h('div', { className: 'rwx-muted' }, 'No runs yet.') : null,
      runs.map(function (r) {
        return h('div', { className: 'rwx-row', key: r.id },
          h('div', { className: 'grow' }, h('div', { className: 't' }, '#' + r.id + ' ' + r.agent + '  \u00b7  ' + r.status), h('div', { className: 's' }, fmtTime(r.started_at) + ' \u00b7 ' + r.folder)),
          h(Btn, { onClick: function () { show(r); } }, 'What changed'),
          r.status === 'running' ? h(Btn, { onClick: function () { end(r); } }, 'End run') : null,
          r.status === 'undone' ? null : h(Btn, { kind: 'warn', onClick: function () { preview(r); } }, 'Undo...'));
      }));
  }

  // ---- Plugins and settings -------------------------------------------------------
  function PluginsTab(p) {
    var a = useState(null), guard = a[0], setGuard = a[1];
    var b = useState(null), st = b[0], setSt = b[1];
    var c = useState(''), maxGb = c[0], setMaxGb = c[1];
    var d = useState(false), auto = d[0], setAuto = d[1];
    var s1 = useState([]), starters = s1[0], setStarters = s1[1];
    var s2 = useState(null), pending = s2[0], setPending = s2[1];
    var s3 = useState(null), shell = s3[0], setShell = s3[1];
    var s4 = useState([]), perrs = s4[0], setPerrs = s4[1];
    function loadStore() {
      invoke('list_starter_plugins').then(setStarters).catch(function () {});
      invoke('get_shell_status').then(setShell).catch(function () {});
      invoke('plugin_errors').then(setPerrs).catch(function () {});
    }
    function install(st) {
      if (pending !== st.id) { setPending(st.id); return; }
      setPending(null);
      invoke('install_starter_plugin', { name: st.id }).then(function () { p.toast(st.name + ' installed and on'); p.reloadPlugins(); loadStore(); }).catch(function (e) { p.toast(msg(e)); });
    }
    function fromDisk() {
      invoke('install_plugin_from_disk').then(function (r) { if (r) { p.toast(r.name + ' installed'); p.reloadPlugins(); loadStore(); } }).catch(function (e) { p.toast(msg(e)); });
    }
    function setShellOn(on) {
      invoke('set_shell_integration', { enabled: on }).then(function () { p.toast(on ? 'Right-click menu added' : 'Right-click menu removed'); loadStore(); }).catch(function (e) { p.toast(msg(e)); });
    }
    function load() {
      loadStore();
      invoke('guard_status').then(setGuard).catch(function () {});
      invoke('get_settings').then(setSt).catch(function () {});
      invoke('cfg_get', { key: 'cfg.disk-guard.max_gb' }).then(function (v) { setMaxGb(v || '5'); });
      invoke('cfg_get', { key: 'cfg.disk-guard.auto_prune' }).then(function (v) { setAuto(v === '1'); });
    }
    useEffect(load, []);
    function toggle(pl) { invoke('set_plugin_enabled', { id: pl.id, enabled: !pl.enabled }).then(p.reloadPlugins).catch(function (e) { p.toast(msg(e)); }); }
    function setting(key, value) {
      var next = Object.assign({}, st); next[key] = value; setSt(next);
      invoke('save_settings', { new: next }).catch(function (e) { p.toast(msg(e)); });
    }
    function saveGuard(gb, ap) {
      invoke('cfg_set', { key: 'cfg.disk-guard.max_gb', value: String(gb) })
        .then(function () { return invoke('cfg_set', { key: 'cfg.disk-guard.auto_prune', value: ap ? '1' : '0' }); })
        .then(function () { return invoke('guard_status'); }).then(setGuard);
    }
    var pct = guard ? Math.min(100, Math.round(100 * guard.used_bytes / Math.max(1, guard.limit_bytes))) : 0;
    return h('div', null,
      h('div', { className: 'rwx-h', style: { marginTop: 0 } }, 'Plugins'),
      p.plugins.map(function (pl) {
        return h('div', { className: 'rwx-row', key: pl.id },
          h('div', { className: 'grow' }, h('div', { className: 't' }, pl.name + (pl.builtin ? '' : '  \u00b7  ' + pl.kind + (pl.version ? ' ' + pl.version : ''))),
            h('div', { className: 's', style: { whiteSpace: 'normal' } }, pl.description),
            !pl.builtin && pl.permissions.length ? h('div', { className: 's', style: { whiteSpace: 'normal' } }, 'Can: ' + pl.permissions.join(' \u00b7 ')) : null),
          pl.builtin ? null : h(Btn, { onClick: function () { invoke('remove_plugin', { id: pl.id }).then(function () { p.reloadPlugins(); loadStore(); }).catch(function (e) { p.toast(msg(e)); }); } }, 'Remove'),
          h('input', { type: 'checkbox', checked: pl.enabled, onChange: function () { toggle(pl); } }));
      }),
      perrs.map(function (e, i) { return h('div', { key: 'pe' + i, className: 'rwx-row', style: { borderColor: '#7f2a31' } }, 'Plugin problem: ' + e); }),
      h('div', { className: 'rwx-h' }, 'Plugin store (bundled)'),
      starters.filter(function (st) { return !p.plugins.some(function (pl) { return pl.id === st.id; }); }).map(function (st) {
        return h('div', { className: 'rwx-row', key: 's' + st.id },
          h('div', { className: 'grow' }, h('div', { className: 't' }, st.name), h('div', { className: 's', style: { whiteSpace: 'normal' } }, st.description),
            pending === st.id ? h('div', { className: 's', style: { whiteSpace: 'normal', color: '#f59e0b' } }, 'It will be able to: ' + st.permissions.join(' \u00b7 ')) : null),
          h(Btn, { kind: pending === st.id ? 'warn' : 'go', onClick: function () { install(st); } }, pending === st.id ? 'Click again to install' : 'Install'));
      }),
      h('div', { className: 'rwx-bar', style: { marginTop: 8 } }, h(Btn, { onClick: fromDisk }, 'Install from a folder...'),
        h(Btn, { onClick: function () { invoke('run_plugin_schedules_now').then(function () { p.toast('Scheduled rules ran'); }); } }, 'Run scheduled rules now')),
      shell && shell.supported ? h('div', null,
        h('div', { className: 'rwx-h' }, 'File manager'),
        h('label', { className: 'rwx-row rwx-sw' }, h('span', null, 'Right-click "Rewind: show history" on files and folders' + (shell.details.length ? ' (' + shell.details[0] + ')' : '')),
          h('input', { type: 'checkbox', checked: shell.installed, onChange: function (e) { setShellOn(e.target.checked); } }))) : null,
      guard ? h('div', null,
        h('div', { className: 'rwx-h' }, 'Disk space guard'),
        h('div', { className: 'rwx-meter' + (guard.over ? ' over' : '') }, h('div', { style: { width: pct + '%' } })),
        h('div', { className: 'rwx-muted', style: { margin: '6px 0 10px' } }, fmtSize(guard.used_bytes) + ' used of ' + fmtSize(guard.limit_bytes) + (guard.over ? ' \u2014 over the limit' : '')),
        h('div', { className: 'rwx-bar' },
          h('span', null, 'Limit (GB)'),
          h('input', { className: 'rwx-in', style: { width: 80 }, value: maxGb, onChange: function (e) { setMaxGb(e.target.value); } }),
          h('label', { className: 'rwx-sw' }, h('span', null, 'Auto-prune when over'),
            h('input', { type: 'checkbox', checked: auto, onChange: function (e) { setAuto(e.target.checked); } })),
          h(Btn, { onClick: function () { saveGuard(parseFloat(maxGb) > 0 ? parseFloat(maxGb) : 5, auto); } }, 'Save'),
          h(Btn, { kind: 'warn', onClick: function () { invoke('guard_prune_now').then(function (r) { p.toast('Freed ' + fmtSize(r.bytes_freed)); load(); }).catch(function (e) { p.toast(msg(e)); }); } }, 'Prune now')),
        h('div', { className: 'rwx-muted' }, 'Pinned versions are never removed.')) : null,
      st ? h('div', null,
        h('div', { className: 'rwx-h' }, 'App settings'),
        [['autostart', 'Start when I log in (hidden, in the tray)'], ['close_to_tray', 'Closing the window keeps recording in the tray'],
         ['auto_update', 'Install updates automatically on start'], ['share_usage', 'Send one anonymous ping a day (helps count users)']].map(function (r) {
          return h('label', { className: 'rwx-row rwx-sw', key: r[0] }, h('span', null, r[1]),
            h('input', { type: 'checkbox', checked: !!st[r[0]], onChange: function (e) {
              if (r[0] === 'autostart') { invoke('set_autostart', { enabled: e.target.checked }).then(function () { setting('autostart', e.target.checked); }).catch(function (er) { p.toast(msg(er)); }); }
              else setting(r[0], e.target.checked);
            } }));
        })) : null);
  }

  // ---- Shell ------------------------------------------------------------------------
  var TABS = [
    ['deleted', 'Deleted files', 'deleted-files'], ['search', 'Search', 'history-search'], ['versions', 'Versions', null],
    ['rewind', 'Rewind folder', 'folder-rewind'], ['labels', 'Labels', 'pins-labels'],
    ['agents', 'AI agents', 'agent-guard'], ['guard', 'Protection', 'tripwire'], ['plugins', 'Plugins & settings', null]
  ];

  function Panel() {
    var a = useState(false), open = a[0], setOpen = a[1];
    var b = useState('deleted'), tab = b[0], setTab = b[1];
    var c = useState([]), plugins = c[0], setPlugins = c[1];
    var d = useState([]), watches = d[0], setWatches = d[1];
    var e2 = useState(null), toastMsg = e2[0], setToast = e2[1];
    var f = useState(''), vpath = f[0], setVpath = f[1];
    var g = useState({ folder: '', to: '' }), rw = g[0], setRw = g[1];
    var k = useState(false), over = k[0], setOver = k[1];
    var tk = useState(0), tick = tk[0], setTick = tk[1];
    var fo = useState(''), fpath = fo[0], setFpath = fo[1];
    var al = useState(0), unseen = al[0], setUnseen = al[1];

    function toast(m) { setToast(m); setTimeout(function () { setToast(null); }, 3500); }
    function reloadPlugins() { invoke('list_plugins').then(setPlugins).catch(function () {}); }
    function has(id) { return plugins.some(function (p) { return p.id === id && p.enabled; }); }
    function openVersions(path) { setVpath(path); setTab('versions'); setOpen(true); }
    function openFolder(path) { setFpath(path); setTab('folder'); setOpen(true); }
    function refreshAlerts() { invoke('list_alerts', { unseenOnly: true }).then(function (l) { setUnseen(l.filter(function (x) { return x.kind === 'tripwire'; }).length); }).catch(function () {}); }
    function openRewind(folder, to) { setRw({ folder: folder, to: to }); setTab('rewind'); setOpen(true); }

    useEffect(function () {
      reloadPlugins();
      var t1 = setInterval(function () {
        invoke('take_launch_file').then(function (f) {
          if (!f) return;
          invoke('path_is_dir', { path: f }).then(function (isDir) { if (isDir) openFolder(f); else openVersions(f); });
        }).catch(function () {});
        refreshAlerts();
        invoke('guard_status').then(function (g) { setOver(g.over); }).catch(function () {});
      }, 1500);
      return function () { clearInterval(t1); };
    }, []);
    useEffect(function () {
      if (!open) return;
      reloadPlugins();
      invoke('get_status').then(function (s) { setWatches(s.watches.map(function (w) { return w.path; })); }).catch(function () {});
      setTick(function (n) { return n + 1; });
    }, [open]);

    var fab = h('button', { className: 'rwx-fab', onClick: function () { setOpen(true); }, title: 'Deleted files, search, folder rewind, plugins' },
      (over || unseen > 0) ? h('span', { className: 'dot' }) : null, 'Tools');
    if (!open) return fab;

    var shared = { watches: watches, toast: toast, has: has, openVersions: openVersions, openRewind: openRewind, tick: tick, refreshAlerts: refreshAlerts };
    var body = null;
    if (tab === 'deleted') body = h(DeletedTab, shared);
    else if (tab === 'search') body = h(SearchTab, shared);
    else if (tab === 'versions') body = h(VersionsTab, Object.assign({ path: vpath }, shared));
    else if (tab === 'rewind') body = h(RewindTab, Object.assign({ key: rw.folder + rw.to, folder: rw.folder, to: rw.to }, shared));
    else if (tab === 'labels') body = h(LabelsTab, shared);
    else if (tab === 'folder') body = h(FolderTab, Object.assign({ key: fpath, folder: fpath }, shared));
    else if (tab === 'agents') body = h(AgentsTab, shared);
    else if (tab === 'guard') body = h(GuardTab, shared);
    else body = h(PluginsTab, Object.assign({ plugins: plugins, reloadPlugins: reloadPlugins }, shared));

    return h('div', null, fab,
      h('div', { className: 'rwx-shade', onClick: function () { setOpen(false); } }),
      h('div', { className: 'rwx-panel' },
        h('div', { className: 'rwx-head' }, h('span', null, 'Rewind tools'), h(Btn, { onClick: function () { setOpen(false); } }, 'Close')),
        over ? h('div', { className: 'rwx-row', style: { margin: '10px 12px 0', borderColor: '#7f2a31' } }, 'History store is over its size limit. See Plugins & settings.') : null,
        unseen > 0 ? h('div', { className: 'rwx-row', style: { margin: '10px 12px 0', borderColor: '#7f2a31', cursor: 'pointer' }, onClick: function () { setTab('guard'); } }, 'Tripwire: many files changed at once. Click to review and undo.') : null,
        h('div', { className: 'rwx-tabs' }, TABS.filter(function (t) { return !t[2] || has(t[2]); }).concat(tab === 'folder' ? [['folder', 'Folder', null]] : []).map(function (t) {
          return h('button', { key: t[0], className: 'rwx-tab' + (tab === t[0] ? ' on' : ''), onClick: function () { setTab(t[0]); } }, t[1]);
        })),
        h('div', { className: 'rwx-body' }, body)),
      toastMsg ? h('div', { className: 'rwx-toast' }, toastMsg) : null);
  }

  var mount = document.createElement('div');
  mount.id = 'rwx-root';
  document.body.appendChild(mount);
  window.ReactDOM.createRoot(mount).render(h(Panel));
})();
