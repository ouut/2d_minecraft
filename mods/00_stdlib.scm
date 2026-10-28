;;; ====================================================================
;;; 00_stdlib.scm: 2D Minecraft / Dwarf Fortress Steel Mod Standard Library
;;; ====================================================================

(mod-log "[stdlib] Loading Steel Scheme Mod Standard Library...")

;; Math & Utility helpers
(define (clamp val min-val max-val)
  (if (< val min-val)
      min-val
      (if (> val max-val)
          max-val
          val)))

;; Helper to build a solid rectangular room / foundation
(define (build-foundation! x1 y1 z1 x2 y2 z2 block-name)
  (fill-box! x1 y1 z1 x2 y2 z2 block-name))

;; Helper to build a hollow room with floor, walls, and ceiling
(define (build-room! x1 y1 z1 x2 y2 z2 wall-block floor-block)
  ;; Solid base floor
  (fill-box! x1 y1 z1 x2 y2 z1 floor-block)
  ;; Walls and ceiling
  (hollow-box! x1 y1 (+ z1 1) x2 y2 z2 wall-block))

;; Helper to clear an open excavation area
(define (excavate! x1 y1 z1 x2 y2 z2)
  (fill-box! x1 y1 z1 x2 y2 z2 "air"))

;; Helper to spawn a cluster of torches for lighting
(define (place-light! x y z)
  (set-block! x y z "torch"))

(mod-log "[stdlib] Steel Scheme Standard Library loaded successfully.")
