use rewind_core::*;
use std::fs;
use std::path::{Path, PathBuf};
use std::thread::sleep;
use std::time::Duration;

fn tmp(tag: &str) -> PathBuf {
    let p = std::env::temp_dir().join(format!("rewind-test-{tag}-{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(&p).unwrap();
    PathBuf::from(normalize_path(&p).unwrap())
}

fn core_in(dir: &Path) -> RewindCore {
    RewindCore::open(dir.join("data")).unwrap()
}

fn text(i: usize) -> String {
    format!("line one of file {i}\n").repeat(80)
}

/// Deterministic pseudo-random bytes (looks like ciphertext to an entropy check).
fn noise(seed: u64, n: usize) -> Vec<u8> {
    let mut x = seed
        .wrapping_mul(6364136223846793005)
        .wrapping_add(1442695040888963407);
    (0..n)
        .map(|_| {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            (x >> 24) as u8
        })
        .collect()
}

#[test]
fn glob_and_spans() {
    assert!(glob_match("*.pdf", "Report.PDF"));
    assert!(glob_match("Screenshot*", "screenshot 2026-11-20.png"));
    assert!(glob_match("a?c", "abc"));
    assert!(!glob_match("*.pdf", "report.pdf.exe"));
    assert_eq!(parse_span("30d"), Some(chrono::Duration::days(30)));
    assert_eq!(parse_span("0d"), None);
    assert_eq!(parse_span("abc"), None);
}

#[test]
fn tripwire_trips_on_burst_and_undo_works() {
    let root = tmp("trip");
    let core = core_in(&root);
    let folder = root.join("docs");
    fs::create_dir_all(&folder).unwrap();
    core.add_watch(&folder).unwrap();
    core.set_tripwire_config(8, 10).unwrap();

    for i in 0..12 {
        fs::write(folder.join(format!("f{i}.txt")), text(i)).unwrap();
        core.save_version(folder.join(format!("f{i}.txt"))).unwrap();
    }
    sleep(Duration::from_millis(30));
    assert!(
        core.list_alerts(false, 10).unwrap().is_empty(),
        "creating files must not trip"
    );

    for i in 0..12 {
        fs::write(folder.join(format!("f{i}.txt")), format!("changed {i}\n")).unwrap();
        core.save_version(folder.join(format!("f{i}.txt"))).unwrap();
    }
    let alerts = core.list_alerts(false, 10).unwrap();
    assert_eq!(alerts.len(), 1, "one alert, not one per file");
    let a = &alerts[0];
    assert_eq!(a.kind, "tripwire");
    assert!(a.window_start.is_some());

    // the pre-burst version of an affected file is pinned
    let hist = core.get_history(folder.join("f0.txt")).unwrap();
    assert!(
        hist.iter().any(|v| v.pinned),
        "last good version should be pinned"
    );
    // and survives an aggressive prune
    core.prune(1, None).unwrap();
    let hist = core.get_history(folder.join("f0.txt")).unwrap();
    assert!(hist.len() >= 2);

    let plan = core.plan_alert_rewind(a.id).unwrap();
    assert_eq!(plan.items.len(), 12);
    let rep = core.apply_alert_rewind(a.id, false).unwrap();
    assert_eq!(rep.restored, 12);
    assert_eq!(fs::read_to_string(folder.join("f3.txt")).unwrap(), text(3));
    assert!(
        core.list_alerts(true, 10).unwrap().is_empty(),
        "alert acknowledged after undo"
    );
}

#[test]
fn tripwire_catches_encryption_quickly() {
    let root = tmp("enc");
    let core = core_in(&root);
    let folder = root.join("docs");
    fs::create_dir_all(&folder).unwrap();
    core.add_watch(&folder).unwrap();
    // default threshold is 50 files, but 5 files turning into noise must be enough
    for i in 0..6 {
        let p = folder.join(format!("n{i}.txt"));
        fs::write(&p, text(i)).unwrap();
        core.save_version(&p).unwrap();
    }
    sleep(Duration::from_millis(30));
    for i in 0..6 {
        let p = folder.join(format!("n{i}.txt"));
        fs::write(&p, noise(i as u64 + 1, 4096)).unwrap();
        core.save_version(&p).unwrap();
    }
    let alerts = core.list_alerts(false, 10).unwrap();
    assert_eq!(alerts.len(), 1);
    assert!(alerts[0].message.contains("encrypted"));
}

#[test]
fn normal_edits_do_not_trip() {
    let root = tmp("calm");
    let core = core_in(&root);
    let folder = root.join("p");
    fs::create_dir_all(&folder).unwrap();
    for i in 0..5 {
        let p = folder.join(format!("c{i}.txt"));
        fs::write(&p, text(i)).unwrap();
        core.save_version(&p).unwrap();
        fs::write(&p, format!("{}\nmore", text(i))).unwrap();
        core.save_version(&p).unwrap();
    }
    assert!(core.list_alerts(false, 10).unwrap().is_empty());
}

#[test]
fn agent_run_report_and_undo() {
    let root = tmp("agent");
    let core = core_in(&root);
    let folder = root.join("proj");
    fs::create_dir_all(&folder).unwrap();
    let (a, b, c) = (
        folder.join("a.rs"),
        folder.join("b.rs"),
        folder.join("c.rs"),
    );
    fs::write(&a, "fn a() {}\n".repeat(100)).unwrap();
    fs::write(&b, "fn b() {}\n").unwrap();
    fs::write(folder.join(".env"), "KEY=1\n").unwrap();

    let run = core
        .agent_start(folder.to_str().unwrap(), "claude", "claude code")
        .unwrap();
    assert_eq!(run.status, "running");

    sleep(Duration::from_millis(20));
    fs::write(&a, "fn a() { gutted }\n").unwrap(); // lost ~100 lines
    fs::remove_file(&b).unwrap();
    fs::write(&c, "fn c() {}\n").unwrap();
    fs::write(folder.join(".env"), "KEY=2\n").unwrap();

    let rep = core.agent_end(run.id).unwrap();
    assert_eq!(
        (rep.created, rep.modified, rep.deleted),
        (1, 2, 1),
        "{:?}",
        rep.changes
    );
    assert!(
        rep.flags.iter().any(|f| f.contains("secrets")),
        "{:?}",
        rep.flags
    );
    assert!(
        rep.flags
            .iter()
            .any(|f| f.contains("a.rs") && f.contains("lost")),
        "{:?}",
        rep.flags
    );

    let plan = core.agent_undo_plan(run.id).unwrap();
    assert!(plan.items.iter().any(|i| i.action == "remove"));
    let undo = core.agent_undo(run.id, true).unwrap();
    assert!(undo.failed.is_empty(), "{:?}", undo.failed);
    assert_eq!(fs::read_to_string(&a).unwrap(), "fn a() {}\n".repeat(100));
    assert_eq!(fs::read_to_string(&b).unwrap(), "fn b() {}\n");
    assert!(!c.exists());
    assert_eq!(fs::read_to_string(folder.join(".env")).unwrap(), "KEY=1\n");
    assert_eq!(core.get_agent_run(run.id).unwrap().status, "undone");
}

#[test]
fn checkpoint_is_throttled() {
    let root = tmp("ckpt");
    let core = core_in(&root);
    let folder = root.join("w");
    fs::create_dir_all(&folder).unwrap();
    fs::write(folder.join("x.txt"), "x").unwrap();
    assert!(core
        .checkpoint(folder.to_str().unwrap(), None, 60)
        .unwrap()
        .is_some());
    assert!(core
        .checkpoint(folder.to_str().unwrap(), None, 60)
        .unwrap()
        .is_none());
    assert!(core
        .checkpoint(folder.to_str().unwrap(), Some("manual"), 0)
        .unwrap()
        .is_some());
}

fn write_plugin(core: &RewindCore, id: &str, toml: &str) {
    let d = core.plugins_dir().join(id);
    fs::create_dir_all(&d).unwrap();
    fs::write(d.join("plugin.toml"), toml).unwrap();
}

#[test]
fn rule_plugin_moves_new_files_and_stays_in_bounds() {
    let root = tmp("plug");
    let core = core_in(&root);
    let dl = root.join("Downloads");
    fs::create_dir_all(&dl).unwrap();
    let dl_s = dl.to_string_lossy().replace('\\', "/");
    write_plugin(
        &core,
        "sorter",
        &format!(
            r#"
id = "sorter"
name = "Sorter"
[permissions]
folders = ["{dl_s}"]
[[rule]]
when = "file.created"
match = "*.pdf"
do = {{ move = "{dl_s}/Docs" }}
[[rule]]
when = "file.created"
match = "*.png"
do = {{ move = "{root_s}/outside" }}
"#,
            root_s = root.to_string_lossy().replace('\\', "/")
        ),
    );
    assert!(core.reload_external_plugins().is_empty());
    let info = core
        .list_plugins()
        .into_iter()
        .find(|p| p.id == "sorter")
        .unwrap();
    assert!(!info.enabled, "a dropped-in plugin starts disabled");
    assert!(!info.builtin);
    assert!(info.permissions.iter().any(|l| l.contains("Move files")));

    // disabled: nothing happens
    let pdf = dl.join("report.pdf");
    fs::write(&pdf, "%PDF fake\n").unwrap();
    core.save_version(&pdf).unwrap();
    assert!(pdf.exists());

    core.set_plugin_enabled("sorter", true).unwrap();
    let pdf2 = dl.join("report2.pdf");
    fs::write(&pdf2, "%PDF fake 2\n").unwrap();
    core.save_version(&pdf2).unwrap();
    assert!(!pdf2.exists());
    assert!(dl.join("Docs").join("report2.pdf").exists());
    // moved files are not lost: the old path is listed as deleted and can be restored
    assert!(core
        .list_deleted(None)
        .unwrap()
        .iter()
        .any(|d| d.name.contains("report2.pdf")));

    // a destination outside the permitted folders is refused and reported
    let png = dl.join("pic.png");
    fs::write(&png, "png-ish\n").unwrap();
    core.save_version(&png).unwrap();
    assert!(png.exists(), "must not move outside the permitted folders");
    assert!(core
        .list_alerts(false, 5)
        .unwrap()
        .iter()
        .any(|a| a.message.contains("Blocked")));
}

#[test]
fn schedule_rule_deletes_old_files_but_keeps_history() {
    let root = tmp("sched");
    let core = core_in(&root);
    let dl = root.join("dl");
    fs::create_dir_all(&dl).unwrap();
    let old = dl.join("setup.exe");
    let fresh = dl.join("new.exe");
    fs::write(&old, "MZ old installer\n").unwrap();
    fs::write(&fresh, "MZ new installer\n").unwrap();
    let f = fs::File::options().write(true).open(&old).unwrap();
    f.set_modified(std::time::SystemTime::now() - Duration::from_secs(40 * 86_400))
        .unwrap();
    drop(f);

    write_plugin(
        &core,
        "cleaner",
        &format!(
            r#"
id = "cleaner"
name = "Cleaner"
[permissions]
folders = ["{}"]
[[rule]]
when = "schedule"
every = "1h"
match = "*.exe"
older_than = "30d"
do = {{ delete = true }}
"#,
            dl.to_string_lossy().replace('\\', "/")
        ),
    );
    core.reload_external_plugins();
    core.set_plugin_enabled("cleaner", true).unwrap();
    core.run_plugin_schedules(true);
    assert!(!old.exists());
    assert!(fresh.exists());
    let back = core.restore_deleted(&old.to_string_lossy()).unwrap();
    assert_eq!(back.path, old.to_string_lossy());
    assert!(old.exists());
}

#[test]
fn manifest_validation_messages() {
    let e = |t: &str| parse_manifest(t).unwrap_err().to_string();
    assert!(e("id = \"x\"\nname = \"X\"").contains("no [[rule]]"));
    assert!(e("id = \"Bad Id\"\nname = \"X\"\n[[rule]]\nwhen=\"schedule\"\nevery=\"1h\"\ndo={notify=\"hi\"}").contains("must be"));
    assert!(e("id = \"x\"\nname = \"X\"\n[permissions]\nfolders=[\"~/a\"]\n[[rule]]\nwhen = \"file.created\"\ndo = { run = [\"x\"] }").contains("exec = true"));
    assert!(e("id = \"x\"\nname = \"X\"\n[permissions]\nfolders=[\"~/a\"]\n[[rule]]\nwhen = \"file.exploded\"\ndo = { notify = \"x\" }").contains("unknown trigger"));
    assert!(e("id = \"x\"\nname = \"X\"\n[permissions]\nfolders=[\"~/a\"]\n[[rule]]\nwhen = \"file.deleted\"\ndo = { delete = true }").contains("still exists"));
    assert!(e("id = \"x\"\nname = \"X\"\n[permissions]\nfolders=[\"~/a\"]\n[[rule]]\nwhen = \"schedule\"\ndo = { notify = \"x\" }").contains("every"));
}

#[test]
fn bundled_plugins_parse_and_install() {
    let root = tmp("starter");
    let core = core_in(&root);
    assert_eq!(core.starter_plugin_infos().len(), STARTER_PLUGINS.len());
    for (name, _) in STARTER_PLUGINS {
        let info = core.install_starter_plugin(name).unwrap();
        assert!(!info.permissions.is_empty());
    }
    let list = core.list_plugins();
    for (name, _) in STARTER_PLUGINS {
        assert!(list
            .iter()
            .any(|p| p.id == *name && p.enabled && !p.builtin));
    }
    core.remove_plugin("screenshot-sorter").unwrap();
    assert!(!core
        .list_plugins()
        .iter()
        .any(|p| p.id == "screenshot-sorter"));
}
