use std::fs::{self, File};
use std::io::Write;
use serde_json::json;

use game_script::{
    json_to_steel_val, steel_val_to_json, PluginContext, PluginError, RegisterFn, ScriptApp,
    ScriptError, ScriptPlugin,
};

// ----------------------------------------------------------------------------
// 1. Domain Commands & Plugin Test
// ----------------------------------------------------------------------------

#[derive(Debug, PartialEq, Eq)]
struct SpawnNpcCommand {
    name: String,
    x: isize,
    y: isize,
}

#[derive(Debug, PartialEq, Eq)]
struct LogMessageCommand(String);

struct GameApiPlugin;

impl ScriptPlugin for GameApiPlugin {
    fn name(&self) -> &'static str {
        "game_api"
    }

    fn build(&self, ctx: &mut PluginContext) -> Result<(), PluginError> {
        let sink = ctx.sink().clone();
        ctx.engine.register_fn(
            "spawn-npc!",
            move |name: String, x: isize, y: isize| {
                sink.emit(SpawnNpcCommand { name, x, y });
            },
        );

        let sink2 = ctx.sink().clone();
        ctx.engine.register_fn("log-msg!", move |msg: String| {
            sink2.emit(LogMessageCommand(msg));
        });

        Ok(())
    }
}

#[test]
fn test_plugin_registration_and_command_sink() {
    let mut app = ScriptApp::new();
    app.add_plugin(GameApiPlugin).expect("Plugin build failed");

    assert_eq!(app.plugins(), &["game_api"]);

    let code = r#"
        (log-msg! "Starting game world")
        (spawn-npc! "Alice" 10 20)
        (spawn-npc! "Bob" 30 40)
    "#;

    app.run(code).expect("Execution failed");

    let cmds = app.drain_commands();
    assert_eq!(cmds.len(), 3);

    let log_cmd = cmds[0].downcast_ref::<LogMessageCommand>().unwrap();
    assert_eq!(log_cmd.0, "Starting game world");

    let npc1 = cmds[1].downcast_ref::<SpawnNpcCommand>().unwrap();
    assert_eq!(
        npc1,
        &SpawnNpcCommand {
            name: "Alice".into(),
            x: 10,
            y: 20,
        }
    );

    let npc2 = cmds[2].downcast_ref::<SpawnNpcCommand>().unwrap();
    assert_eq!(
        npc2,
        &SpawnNpcCommand {
            name: "Bob".into(),
            x: 30,
            y: 40,
        }
    );

    // Command sink is now drained
    assert!(app.drain_commands().is_empty());
}

// ----------------------------------------------------------------------------
// 2. REPL Evaluation & Echo
// ----------------------------------------------------------------------------

#[test]
fn test_eval_repl_and_echo() {
    let mut app = ScriptApp::new();

    let res1 = app.eval_repl("(+ 10 25)").unwrap();
    assert_eq!(res1, "=> 35");

    let res2 = app.eval_repl("(string-append \"Hello, \" \"Antigravity\")").unwrap();
    assert_eq!(res2, "=> \"Hello, Antigravity\"");

    let _ = app.eval_repl("(define x 42)").unwrap();
    let res3 = app.eval_repl("(* x 2)").unwrap();
    assert_eq!(res3, "=> 84");

    let res4 = app.eval_repl("(list 1 2 3)").unwrap();
    assert_eq!(res4, "=> (1 2 3)");
}

// ----------------------------------------------------------------------------
// 3. Safe Eval Watchdog Timeout (Prevent Infinite Loop)
// ----------------------------------------------------------------------------

#[test]
fn test_safe_eval_watchdog_timeout() {
    let mut app = ScriptApp::new();

    // Infinite loop in Scheme: (let loop () (loop))
    let start = std::time::Instant::now();
    let res = app.safe_eval("(let loop () (loop))", 150);
    let elapsed = start.elapsed();

    assert!(res.is_err(), "Expected timeout error on infinite loop");
    match res.err().unwrap() {
        ScriptError::Timeout(ms) => assert_eq!(ms, 150),
        other => panic!("Expected ScriptError::Timeout, got {:?}", other),
    }

    // Ensure timeout stopped within reasonable bounds (< 1000ms)
    assert!(elapsed.as_millis() < 1000);

    // Verify VM engine was properly resumed and can still evaluate normally
    let res_after = app.safe_eval("(+ 100 200)", 500);
    assert_eq!(res_after.unwrap(), "=> 300");
}

// ----------------------------------------------------------------------------
// 4. SteelVal <-> JSON Bidirectional Interoperability
// ----------------------------------------------------------------------------

#[test]
fn test_steel_val_json_interop() {
    let complex_json = json!({
        "item_id": 4096,
        "name": "enchanted_pickaxe",
        "durability": 1561.75,
        "is_magical": true,
        "attributes": null,
        "lore": [
            "Forged in ancient stars",
            "Mines anything instantly"
        ],
        "modifiers": {
            "efficiency": 5,
            "fortune": 3
        }
    });

    // 1. JSON -> SteelVal
    let steel_val = json_to_steel_val(&complex_json);

    // 2. SteelVal -> JSON
    let roundtrip_json = steel_val_to_json(&steel_val).expect("Failed to convert to json");

    assert_eq!(complex_json, roundtrip_json);

    // 3. Scheme-created values -> JSON
    let mut app = ScriptApp::new();
    let vals = app
        .run(r#"
            (hash "block" "obsidian" "blast_resistance" 1200 "transparent" #f)
        "#)
        .unwrap();

    let block_json = steel_val_to_json(&vals[0]).unwrap();
    assert_eq!(block_json["block"], "obsidian");
    assert_eq!(block_json["blast_resistance"], 1200);
    assert_eq!(block_json["transparent"], false);
}

// ----------------------------------------------------------------------------
// 5. ScriptWatcher & Auto Reload Detection
// ----------------------------------------------------------------------------

#[test]
fn test_script_watcher_and_reload() {
    let test_dir = std::env::temp_dir().join("test_game_script_reload");
    let _ = fs::remove_dir_all(&test_dir);
    fs::create_dir_all(&test_dir).unwrap();

    let script_path = test_dir.join("config.scm");
    {
        let mut file = File::create(&script_path).unwrap();
        writeln!(file, "(define max-players 16)").unwrap();
    }

    let mut app = ScriptApp::new();
    app.watch_dir(&test_dir);

    // First load via check_reload
    // Since ScriptWatcher initializes with the snapshot, modifying triggers reload
    std::thread::sleep(std::time::Duration::from_millis(50));
    {
        let mut file = File::create(&script_path).unwrap();
        writeln!(file, "(define max-players 64)").unwrap();
    }

    assert!(app.check_reload().unwrap());

    // Verify Scheme environment has the updated value
    let res = app.eval_repl("max-players").unwrap();
    assert_eq!(res, "=> 64");

    // Second check without modification should return false
    assert!(!app.check_reload().unwrap());

    // Clean up
    let _ = fs::remove_dir_all(&test_dir);
}
