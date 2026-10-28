;;; ====================================================================
;;; 02_structures_and_tools.scm: Procedural Outpost Generator & God Tools
;;; ====================================================================

(mod-log "[tools] Registering custom God Tools and Structure Generators...")

;; Tool 1: "Build Watchtower" - Spawns a 3-layer stone & plank defensive tower with stairs & torches
(define (tool-build-watchtower x y z)
  (mod-log (string-append "[Tool] Erecting watchtower at (" (number->string x) "," (number->string y) "," (number->string z) ")"))
  
  ;; 1. Cobblestone base (5x5, height 4)
  (fill-box! (- x 2) (- y 2) z (+ x 2) (+ y 2) (+ z 3) "cobblestone")
  ;; Hollow out inside
  (fill-box! (- x 1) (- y 1) (+ z 1) (+ x 1) (+ y 1) (+ z 3) "air")
  
  ;; 2. Wood Planks upper floor (7x7 balcony, height 3)
  (fill-box! (- x 3) (- y 3) (+ z 4) (+ x 3) (+ y 3) (+ z 4) "planks")
  (hollow-box! (- x 3) (- y 3) (+ z 5) (+ x 3) (+ y 3) (+ z 6) "glass")
  (fill-box! (- x 2) (- y 2) (+ z 5) (+ x 2) (+ y 2) (+ z 6) "air")
  
  ;; 3. Roof & parapets
  (fill-box! (- x 3) (- y 3) (+ z 7) (+ x 3) (+ y 3) (+ z 7) "planks")
  
  ;; 4. Torches on the 4 corners of the balcony
  (set-block! (- x 3) (- y 3) (+ z 8) "torch")
  (set-block! (+ x 3) (- y 3) (+ z 8) "torch")
  (set-block! (- x 3) (+ y 3) (+ z 8) "torch")
  (set-block! (+ x 3) (+ y 3) (+ z 8) "torch")
  
  ;; 5. Add stairs climbing to top floor
  (set-block! (- x 1) y (+ z 1) "stair")
  (set-block! (- x 1) y (+ z 2) "stair")
  (set-block! (- x 1) y (+ z 3) "stair")
  (set-block! (- x 1) y (+ z 4) "stair")
  
  ;; 6. Spawn a guard dwarf colonist atop the tower
  (spawn-colonist! x y (+ z 5) "Tower Watchman")
  
  "Watchtower constructed successfully!")

(register-tool! "Build Watchtower" "tool-build-watchtower")

;; Tool 2: "Meteor Strike" - Carves a 5x5 crater, replaces ground with obsidian and fills core with lava
(define (tool-meteor-strike x y z)
  (mod-log (string-append "[Tool] Meteor impact at (" (number->string x) "," (number->string y) "," (number->string z) ")!"))
  
  ;; Blast crater
  (fill-box! (- x 2) (- y 2) (- z 1) (+ x 2) (+ y 2) (+ z 2) "air")
  ;; Scorched obsidian ring
  (fill-box! (- x 2) (- y 2) (- z 2) (+ x 2) (+ y 2) (- z 2) "obsidian")
  ;; Molten core
  (spawn-fluid! x y (- z 1) "lava" 7)
  (set-block! (+ x 1) y (- z 1) "fire" 0)
  
  "Meteor impact completed!")

(register-tool! "Meteor Strike" "tool-meteor-strike")

;; Tool 3: "Colonist Outpost Call" - Teleports player and spawns 2 dwarf builders
(define (tool-call-reinforcements x y z)
  (spawn-colonist! (+ x 1) y z "Dwarf Miner")
  (spawn-colonist! x (+ y 1) z "Dwarf Engineer")
  (designate-box! (- x 3) (- y 3) z (+ x 3) (+ y 3) z "mine" "air")
  "Reinforcements deployed and mining area marked!")

(register-tool! "Call Colonists" "tool-call-reinforcements")

(mod-log "[tools] 3 custom tools registered in God Mode toolbar.")
