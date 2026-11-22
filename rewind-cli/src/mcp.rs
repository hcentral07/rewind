//! `rewind mcp`: a Model Context Protocol server over stdio, so an AI coding agent can protect
//! itself. Newline-delimited JSON-RPC 2.0, no extra dependencies.
//!
//! Register it with an agent, for example:
//!   claude mcp add rewind -- rewind mcp
use anyhow::Result;
use rewind_core::{RewindCore, WatcherHandle};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::io::{BufRead, Write};

const PROTOCOL: &str = "2024-11-05";

fn tools() -> Value {
    let folder = json!({"type": "string", "description": "Absolute path of the project folder"});
    json!([
        {
            "name": "rewind_begin_session",
            "description": "Call this BEFORE you start editing a project. Saves a checkpoint of the folder and starts recording everything you change, so the whole session can be reviewed or undone. Returns a session id.",
            "inputSchema": {"type": "object", "properties": {
                "folder": folder, "agent": {"type": "string", "description": "Your name, e.g. claude-code"}},
                "required": ["folder"]}
        },
        {
            "name": "rewind_session_report",
            "description": "Lists the files created, modified and deleted since the session began, with line counts and warnings (mass deletes, secrets touched, files gutted). Use it to double-check your own work.",
            "inputSchema": {"type": "object", "properties": {
                "session_id": {"type": "integer", "description": "Id from rewind_begin_session; omit for the latest session"}}}
        },
        {
            "name": "rewind_undo_session",
            "description": "Puts the folder back to how it was when the session began. Without confirm=true it only shows what would change. Files you created are removed only with remove_new=true.",
            "inputSchema": {"type": "object", "properties": {
                "session_id": {"type": "integer"},
                "confirm": {"type": "boolean"},
                "remove_new": {"type": "boolean"}}}
        },
        {
            "name": "rewind_checkpoint",
            "description": "Save a labelled checkpoint of a folder right now (cheap, skipped if one was saved in the last 2 minutes unless force=true). Call before a risky refactor, a mass rename or a delete.",
            "inputSchema": {"type": "object", "properties": {
                "folder": folder, "name": {"type": "string"}, "force": {"type": "boolean"}},
                "required": ["folder"]}
        },
        {
            "name": "rewind_file_history",
            "description": "Version history of one file: version numbers, times and what changed.",
            "inputSchema": {"type": "object", "properties": {"path": {"type": "string"}}, "required": ["path"]}
        },
        {
            "name": "rewind_restore_file",
            "description": "Restore one file to a version number from rewind_file_history. The current content is saved first, so this can be undone.",
            "inputSchema": {"type": "object", "properties": {
                "path": {"type": "string"}, "version": {"type": "integer"}}, "required": ["path", "version"]}
        },
        {
            "name": "rewind_deleted_files",
            "description": "Files that disappeared from watched folders and can be brought back; pass restore to bring one back.",
            "inputSchema": {"type": "object", "properties": {
                "folder": {"type": "string"}, "restore": {"type": "string", "description": "Full path of a deleted file to restore"}}}
        }
    ])
}

struct Server {
    core: RewindCore,
    watchers: HashMap<String, WatcherHandle>,
}

impl Server {
    fn watch(&mut self, folder: &str) {
        if let Ok(key) = self.core.add_watch(folder) {
            if !self.watchers.contains_key(&key) {
                if let Ok(h) = self.core.start_watcher(&key) {
                    self.watchers.insert(key, h);
                }
            }
        }
    }

    fn run_id(&self, args: &Value) -> Result<i64> {
        if let Some(id) = args.get("session_id").and_then(|v| v.as_i64()) {
            return Ok(id);
        }
        self.core
            .latest_agent_run(None)?
            .map(|r| r.id)
            .ok_or_else(|| anyhow::anyhow!("no session yet: call rewind_begin_session first"))
    }

    fn call(&mut self, name: &str, a: &Value) -> Result<String> {
        let s = |k: &str| a.get(k).and_then(|v| v.as_str()).map(String::from);
        let b = |k: &str| a.get(k).and_then(|v| v.as_bool()).unwrap_or(false);
        match name {
            "rewind_begin_session" => {
                let folder = s("folder").ok_or_else(|| anyhow::anyhow!("folder is required"))?;
                let run = self
                    .core
                    .agent_start(&folder, &s("agent").unwrap_or_default(), "mcp")?;
                self.watch(&run.folder);
                Ok(format!(
                    "Session {} started for {}. Checkpoint saved. Changes are being recorded. Call rewind_session_report to review, rewind_undo_session to roll back.",
                    run.id, run.folder
                ))
            }
            "rewind_session_report" => {
                let id = self.run_id(a)?;
                let r = self.core.agent_report_live(id)?;
                Ok(crate::render_report(&r))
            }
            "rewind_undo_session" => {
                let id = self.run_id(a)?;
                if !b("confirm") {
                    let plan = self.core.agent_undo_plan(id)?;
                    let mut out = format!(
                        "DRY RUN for session {id}: {} files would change ({} already match). Call again with confirm=true to do it.\n",
                        plan.items.len(),
                        plan.unchanged
                    );
                    for it in plan.items.iter().take(50) {
                        out.push_str(&format!("  {:<8} {}\n", it.action, it.path));
                    }
                    return Ok(out);
                }
                let rep = self.core.agent_undo(id, b("remove_new"))?;
                Ok(format!(
                    "Rolled back session {id}: restored {}, recreated {}, removed {}{}",
                    rep.restored,
                    rep.recreated,
                    rep.removed,
                    if rep.failed.is_empty() {
                        String::new()
                    } else {
                        format!(", {} failed: {}", rep.failed.len(), rep.failed.join("; "))
                    }
                ))
            }
            "rewind_checkpoint" => {
                let folder = s("folder").ok_or_else(|| anyhow::anyhow!("folder is required"))?;
                let every = if b("force") { 0 } else { 120 };
                match self.core.checkpoint(&folder, s("name").as_deref(), every)? {
                    Some(l) => {
                        self.watch(&l.folder);
                        Ok(format!("Checkpoint saved: {}", l.name))
                    }
                    None => Ok("A checkpoint was saved less than 2 minutes ago; skipped (use force=true to save another).".into()),
                }
            }
            "rewind_file_history" => {
                let p = s("path").ok_or_else(|| anyhow::anyhow!("path is required"))?;
                let h = self.core.get_history(&p)?;
                if h.is_empty() {
                    return Ok("No history for that file (is it inside a watched folder?)".into());
                }
                let mut out = format!("{}\n", h[0].path);
                for v in h.iter().take(40) {
                    out.push_str(&format!(
                        "v{} {} +{} -{} {}\n",
                        v.number, v.created_at, v.lines_added, v.lines_deleted, v.summary
                    ));
                }
                Ok(out)
            }
            "rewind_restore_file" => {
                let p = s("path").ok_or_else(|| anyhow::anyhow!("path is required"))?;
                let n = a
                    .get("version")
                    .and_then(|v| v.as_i64())
                    .ok_or_else(|| anyhow::anyhow!("version is required"))?;
                let h = self.core.get_history(&p)?;
                let v = h
                    .iter()
                    .find(|v| v.number == n)
                    .ok_or_else(|| anyhow::anyhow!("no version v{n}"))?;
                self.core.restore_version(v.id)?;
                Ok(format!("Restored {} to v{n}", v.path))
            }
            "rewind_deleted_files" => {
                if let Some(p) = s("restore") {
                    let v = self.core.restore_deleted(&p)?;
                    return Ok(format!("Restored {}", v.path));
                }
                let list = self.core.list_deleted(s("folder").as_deref())?;
                if list.is_empty() {
                    return Ok("No deleted files.".into());
                }
                Ok(list
                    .iter()
                    .take(100)
                    .map(|d| format!("{}  {}", d.deleted_at, d.path))
                    .collect::<Vec<_>>()
                    .join("\n"))
            }
            other => Err(anyhow::anyhow!("unknown tool {other}")),
        }
    }
}

pub fn serve(core: RewindCore) -> Result<()> {
    let mut server = Server {
        core,
        watchers: HashMap::new(),
    };
    let stdin = std::io::stdin();
    let mut out = std::io::stdout();
    for line in stdin.lock().lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let Ok(req) = serde_json::from_str::<Value>(&line) else {
            continue;
        };
        let id = req.get("id").cloned();
        let method = req.get("method").and_then(|m| m.as_str()).unwrap_or("");
        let params = req.get("params").cloned().unwrap_or(Value::Null);

        let reply = match method {
            "initialize" => Some(Ok(json!({
                "protocolVersion": params.get("protocolVersion").and_then(|v| v.as_str()).unwrap_or(PROTOCOL),
                "capabilities": {"tools": {}},
                "serverInfo": {"name": "rewind", "version": env!("CARGO_PKG_VERSION")},
                "instructions": "Call rewind_begin_session before editing a project so your changes can be reviewed and undone."
            }))),
            "ping" => Some(Ok(json!({}))),
            "tools/list" => Some(Ok(json!({"tools": tools()}))),
            "tools/call" => {
                let name = params.get("name").and_then(|n| n.as_str()).unwrap_or("");
                let args = params.get("arguments").cloned().unwrap_or(json!({}));
                Some(Ok(match server.call(name, &args) {
                    Ok(text) => json!({"content": [{"type": "text", "text": text}]}),
                    Err(e) => {
                        json!({"content": [{"type": "text", "text": e.to_string()}], "isError": true})
                    }
                }))
            }
            m if m.starts_with("notifications/") => None,
            _ => id
                .as_ref()
                .map(|_| Err((-32601, format!("method not found: {method}")))),
        };

        if let (Some(id), Some(reply)) = (id, reply) {
            let msg = match reply {
                Ok(result) => json!({"jsonrpc": "2.0", "id": id, "result": result}),
                Err((code, message)) => {
                    json!({"jsonrpc": "2.0", "id": id, "error": {"code": code, "message": message}})
                }
            };
            writeln!(out, "{msg}")?;
            out.flush()?;
        }
    }
    Ok(())
}
