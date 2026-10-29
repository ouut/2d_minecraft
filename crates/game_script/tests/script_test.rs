use std::fs::{self, File};
use std::io::Write;
use serde_json::json;

use game_script::{
    json_to_steel_val, steel_val_to_json, PluginContext, PluginError, RegisterFn, ScriptApp,
    ScriptError, ScriptPlugin, SteelVal,
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

// ----------------------------------------------------------------------------
// 6. Precompiled Native Function Calling
// ----------------------------------------------------------------------------

#[test]
fn test_precompiled_function_call() {
    let mut app = ScriptApp::new();

    app.run(r#"
        (define (multiply a b)
          (* a b))

        (define (greet name title)
          (string-append "Hello, " title " " name "!"))
    "#).unwrap();

    // 1. Direct call_function without string compilation
    let res1 = app.call_function("multiply", vec![SteelVal::IntV(6), SteelVal::IntV(7)]).unwrap();
    assert_eq!(res1, SteelVal::IntV(42));

    let res2 = app.call_function("greet", vec![
        SteelVal::StringV("Antigravity".into()),
        SteelVal::StringV("Architect".into()),
    ]).unwrap();
    assert_eq!(res2, SteelVal::StringV("Hello, Architect Antigravity!".into()));

    // 2. safe_call_function timeout protection on infinite loop
    app.run(r#"
        (define (spin-forever)
          (let loop () (loop)))
    "#).unwrap();

    let start = std::time::Instant::now();
    let timeout_res = app.safe_call_function("spin-forever", vec![], 100);
    let elapsed = start.elapsed();

    assert!(timeout_res.is_err());
    match timeout_res.err().unwrap() {
        ScriptError::Timeout(ms) => assert_eq!(ms, 100),
        other => panic!("Expected Timeout, got {:?}", other),
    }
    assert!(elapsed.as_millis() < 1000);

    // 3. Immediately call normal function again to verify engine and watchdog recovery
    let recover_res = app.safe_call_function("multiply", vec![SteelVal::IntV(10), SteelVal::IntV(5)], 200).unwrap();
    assert_eq!(recover_res, SteelVal::IntV(50));
}

// ----------------------------------------------------------------------------
// 7. Dual-Paradigm: Scheme-Authoritative World State & Snapshots
// ----------------------------------------------------------------------------

#[test]
fn test_scheme_authoritative_paradigm() {
    let mut app = ScriptApp::new();

    // Scheme controls the authoritative game state
    app.run(r#"
        (define *world* (hash
          "tick_count" 0
          "player" (hash "x" 100.0 "y" 200.0 "vx" 5.0 "vy" -2.0 "hp" 100)
        ))

        (define (game-tick dt)
          (let* ([tc (+ (hash-ref *world* "tick_count") 1)]
                 [p (hash-ref *world* "player")]
                 [px (+ (hash-ref p "x") (* (hash-ref p "vx") dt))]
                 [py (+ (hash-ref p "y") (* (hash-ref p "vy") dt))]
                 [new-p (hash-insert (hash-insert p "x" px) "y" py)])
            (set! *world* (hash-insert (hash-insert *world* "tick_count" tc) "player" new-p))
            *world*))
    "#).unwrap();

    // Drive 5 ticks from Rust main loop
    for _ in 0..5 {
        let _ = app.tick("game-tick", 1.0).unwrap();
    }

    // Extract snapshot
    let snapshot = app.extract_snapshot("*world*").unwrap();
    assert_eq!(snapshot["tick_count"], 5);
    assert_eq!(snapshot["player"]["x"], 125.0);
    assert_eq!(snapshot["player"]["y"], 190.0);
    assert_eq!(snapshot["player"]["hp"], 100);
}

// ----------------------------------------------------------------------------
// 8. World State Serialization & Disk File Restore
// ----------------------------------------------------------------------------

#[test]
fn test_state_serialization_and_disk_restore() {
    let temp_save_file = std::env::temp_dir().join("game_script_save_test.json");
    if temp_save_file.exists() {
        let _ = fs::remove_file(&temp_save_file);
    }

    let mut app1 = ScriptApp::new();
    app1.run(r#"
        (define world-state (hash
          "level" "nether_fortress"
          "score" 9999
          "inventory" (list "diamond_pickaxe" "golden_apple" "ender_pearl")
          "unlocked_achievements" (hash "mine_wood" #t "craft_portal" #t)
        ))
    "#).unwrap();

    // 1. Dump to JSON value and verify
    let dumped_val = app1.dump_state_json("world-state").unwrap();
    assert_eq!(dumped_val["level"], "nether_fortress");
    assert_eq!(dumped_val["score"], 9999);
    assert_eq!(dumped_val["inventory"][0], "diamond_pickaxe");

    // 2. Dump directly to file
    app1.dump_state_to_file("world-state", &temp_save_file).unwrap();
    assert!(temp_save_file.exists());

    // 3. Create fresh new VM instance (app2) and restore from file
    let mut app2 = ScriptApp::new();
    app2.load_state_from_file("restored-world", &temp_save_file).unwrap();

    // Verify Scheme code can seamlessly read the restored data
    let level_res = app2.eval_repl("(hash-ref restored-world \"level\")").unwrap();
    assert_eq!(level_res, "=> \"nether_fortress\"");

    let score_res = app2.eval_repl("(hash-ref restored-world \"score\")").unwrap();
    assert_eq!(score_res, "=> 9999");

    let inv_res = app2.eval_repl("(first (hash-ref restored-world \"inventory\"))").unwrap();
    assert_eq!(inv_res, "=> \"diamond_pickaxe\"");

    // 4. Modify state in Scheme, dump again with load_state_json
    let new_json = json!({
        "level": "the_end",
        "score": 15000,
        "boss_alive": true
    });
    app2.load_state_json("active-quest", &new_json).unwrap();
    let quest_res = app2.eval_repl("(hash-ref active-quest \"level\")").unwrap();
    assert_eq!(quest_res, "=> \"the_end\"");

    // Cleanup
    let _ = fs::remove_file(&temp_save_file);
}

// ----------------------------------------------------------------------------
// 9. Load Path & Relative Search Stack
// ----------------------------------------------------------------------------

#[test]
fn test_load_paths_and_multi_file_require() {
    let base_dir = std::env::temp_dir().join("test_game_script_load_path");
    let modules_dir = base_dir.join("modules");
    let _ = fs::remove_dir_all(&base_dir);
    fs::create_dir_all(&modules_dir).unwrap();

    // Create a module file: modules/math_utils.scm
    let module_file = modules_dir.join("math_utils.scm");
    {
        let mut file = File::create(&module_file).unwrap();
        writeln!(file, r#"
            (provide cube)
            (define (cube x) (* x x x))
        "#).unwrap();
    }

    // Create main script file: base_dir/main.scm
    let main_file = base_dir.join("main.scm");
    {
        let mut file = File::create(&main_file).unwrap();
        writeln!(file, r#"
            (require "math_utils.scm")
            (define result (cube 3))
        "#).unwrap();
    }

    let mut app = ScriptApp::new();
    app.with_load_path(&modules_dir);
    assert_eq!(app.load_paths().len(), 1);

    // Resolve path check
    let resolved = app.resolve_path("math_utils.scm");
    assert!(resolved.is_some());
    assert_eq!(resolved.unwrap(), module_file);

    // Run file with module requirement
    app.run_file(&main_file).unwrap();

    let res = app.eval_repl("result").unwrap();
    assert_eq!(res, "=> 27");

    // Clean up
    let _ = fs::remove_dir_all(&base_dir);
}

// ----------------------------------------------------------------------------
// 10. Persistent Watchdog High Frequency Reuse Stress Test
// ----------------------------------------------------------------------------

#[test]
fn test_persistent_watchdog_stress_and_no_thread_spawn() {
    let mut app = ScriptApp::new();

    app.run(r#"
        (define (compute-step x)
          (+ (* x 2) 1))
    "#).unwrap();

    // Rapidly execute 200 safe calls with 100ms timeout
    let start = std::time::Instant::now();
    for i in 0..200 {
        let res = app.safe_call_function("compute-step", vec![SteelVal::IntV(i)], 100).unwrap();
        assert_eq!(res, SteelVal::IntV(i * 2 + 1));
    }
    let elapsed = start.elapsed();

    // 200 guarded calls should finish in a few tens of milliseconds, not seconds
    assert!(elapsed.as_millis() < 2000, "200 safe calls took too long: {:?}", elapsed);
}
