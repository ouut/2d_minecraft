# 2.5D Layered Voxel Minecraft Engine (Dwarf Fortress × Minecraft)

A high-performance 2.5D isometric voxel engine written in Rust with true 3D voxel data, 4-direction 90° camera rotation, layered Z-slicing editing, dual-channel BFS lighting, cellular automata fluids, structural collapse physics, 3D A* pathfinding, and full Steel Scheme modding support.

## Architecture

The project follows a strict **3-Tier Star-Tree DAG** Cargo Workspace:
```
crates/
├── voxel_core/    # Layer 0: Voxel, Chunk, IsoCamera, WorldAccess trait, EngineCommand bus
├── voxel_gen/     # Layer 1: Minecraft-style 3D density noise, biomes, caves, ores, ramps
├── voxel_sim/     # Layer 1: Dual-channel BFS lighting, fluids, structural collapse
├── voxel_ecs/     # Layer 1: hecs ECS, 3D cross-layer A* pathfinding, colony jobs, player
├── voxel_script/  # Layer 1: Steel Scheme VM modding engine, REPL, F5 hot-reloading
├── voxel_render/  # Layer 1: Procedural 2.5D pixel-art atlas, painter's algorithm renderer, HUD
└── voxel_app/     # Layer 2: Main loop, window, dynamic chunk world, input routing
```

## Documentation

Detailed architecture specifications and multi-agent workflow documents are in `docs/`:
- [docs/implementation_plan.md](docs/implementation_plan.md)
- [docs/agent_concurrency_workflow.md](docs/agent_concurrency_workflow.md)

## Controls & Shortcuts

| Shortcut | Action |
| :--- | :--- |
| **F1** | Switch to **Player Survival Mode** (WASD movement, Left-click mine, Right-click place) |
| **F2** | Switch to **Colony Simulation Mode** (Drag box to designate mining/building jobs) |
| **F3** | Switch to **God Editor Mode** (Paint terrain, spawn fluids, trigger Mod Tools) |
| **R** | Rotate 2.5D Isometric Camera 90° clockwise (North -> East -> South -> West) |
| **Q / E** / Mouse Wheel | Adjust current Z-slice height (layers above are hidden/cut away) |
| **F5** | **Hot-Reload Steel Scheme Mods** from `mods/*.scm` without restarting |
| **`** / **F12** | Toggle interactive in-game **Steel Scheme REPL console** |

## Steel Scheme Modding (`mods/*.scm`)

Mods placed in `mods/` are automatically loaded on boot and can be hot-reloaded with `F5`.

Example mod scripts included:
- `mods/00_stdlib.scm`: Standard geometry and excavation macros
- `mods/01_magic_blocks.scm`: Registers custom ores (`mythril_ore`, `aether_crystal`, `quicksand`, `magma_slime`)
- `mods/02_structures_and_tools.scm`: Registers custom interactive God Tools (`Build Watchtower`, `Meteor Strike`, `Call Colonists`)
