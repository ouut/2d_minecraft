;;; ====================================================================
;;; 01_magic_blocks.scm: Custom Fantasy Ores, Crystals & Hazard Flora
;;; ====================================================================

(mod-log "[magic_blocks] Registering custom mod blocks...")

;; 1. Mythril Ore: Cyan glowing rare ore with light emission 8
(register-block!
  "mythril_ore"       ; name
  90 220 255          ; top RGB
  60 180 220          ; left RGB
  40 140 180          ; right RGB
  9                   ; light emission (0-15)
  #t                  ; is_solid
  #f                  ; is_transparent
  #f                  ; is_liquid
  #f)                 ; has_gravity

;; 2. Aether Crystal: High-radiance violet power source
(register-block!
  "aether_crystal"
  210 130 255
  170 90 220
  130 60 180
  14
  #t
  #t
  #f
  #f)

;; 3. Quicksand: Hazardous crumbling sand that flows like liquid and falls with gravity
(register-block!
  "quicksand"
  180 160 110
  150 130 90
  120 105 70
  0
  #f
  #f
  #t
  #t)

;; 4. Magma Slime Block: Bouncy orange glowing hazard block
(register-block!
  "magma_slime"
  255 140 30
  220 110 20
  180 80 10
  10
  #t
  #t
  #f
  #f)

(mod-log "[magic_blocks] 4 custom mod blocks registered (mythril_ore, aether_crystal, quicksand, magma_slime).")
