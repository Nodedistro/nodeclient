# NodeEngine: native Rust engine

NodeEngine is Nodedistro's original voxel engine, built in this repository without Horizon code or Mojang assets. This is the first playable milestone toward Minecraft compatibility, **not a replacement Minecraft client yet**. It runs directly as a Windows executable without a JVM. Macroquad supplies window/input/graphics; the terrain, voxel mesher, simulation, interaction, and save handling are our Rust code.

## Run

From the repository root:

```powershell
pnpm engine
# Or directly:
cargo run -p nodeengine --release
```

`pnpm engine:build` creates `target/release/nodeengine.exe`. The executable can run by itself. It initially shows a paused world; click to begin. The existing NodeClient Java launch flow is unchanged, and the engine is not yet included in the NodeClient installer.

- WASD: move; mouse: look; Space: jump; Shift: sprint.
- Left click: remove the targeted block; right click: place a block.
- 1–7: grass, dirt, stone, sand, oak log, leaves, planks.
- Escape: pause/release the pointer; F5: retry saving; Q while paused: save and quit.
- The bottom layer and world boundaries bound the sandbox. Blocks cannot be placed inside the player.

The initial sandbox is 64 × 48 × 64 blocks, built from a seed, with trees. Terrain is our own generation algorithm, not Minecraft seed parity. Visible faces are batched by 16 × 16 columns and only affected columns and their neighbors are rebuilt after edits. Simulation uses fixed 120 Hz steps with axis-separated player collision and exact voxel ray traversal.

## Saves

The default world is `%LOCALAPPDATA%/Nodedistro/NodeEngine/worlds/first-world.json`. Each successful edit is saved atomically; failed saves show a retry message. Only blocks/seed are saved in version 1; the player respawns on load. A process lock prevents two engines from editing the same world concurrently. A bad save causes an error rather than overwriting it.

```powershell
pnpm engine --world C:/Games/NodeEngine/my-world.json --seed 123
```

Use a new path for a new seed. Existing worlds load their stored seed and blocks. These JSON files are our own format; never point the engine at a Minecraft `level.dat` or region file.

## Minecraft structure interchange: limited Sponge v2 support

```powershell
# Import into a NEW NodeEngine world. The input file is read-only.
pnpm engine --world C:/Games/NodeEngine/imported.json --import-schematic C:/Structures/house.schem

# Export a block volume, then exit without opening a window.
pnpm engine --world C:/Games/NodeEngine/my-world.json --export-schematic C:/Structures/export.schem
```

Implemented against [Sponge Schematic Specification v2](https://github.com/SpongePowered/Schematic-Specification/blob/master/versions/schematic-2.md): gzip NBT, a block-state palette, VarInt indices, and X/Z/Y block ordering. Export uses data version 3465 (Minecraft 1.20.1 block-state names). The full sandbox is exported; the output must be a new file. Imported structures are centered horizontally at Y=0; source offsets are deliberately not applied.

The initial supported palette is:

| State | Support |
| --- | --- |
| `minecraft:air`, `minecraft:dirt`, `minecraft:stone`, `minecraft:sand`, `minecraft:oak_planks` | Exact state |
| `minecraft:grass_block[snowy=false]` | Also accepts the unqualified default state |
| `minecraft:oak_log[axis=y]` | Vertical logs only; also accepts the default state |
| `minecraft:oak_leaves[distance=7,persistent=true,waterlogged=false]` | This state only; also accepts the default state |

Unsupported states, entities, block entities, biome data, oversized structures, and malformed data are rejected explicitly. The importer does not silently substitute unknown blocks. Versions 1/3, `.schematic`, `.mca`, and whole Minecraft world directories are not supported. Tests cover the format and internal round-trip; import into a live WorldEdit installation has not yet been verified.

## Verification

```powershell
pnpm engine:test
cargo clippy -p nodeengine --all-targets -- -D warnings
cargo run -p nodeengine -- --world .verification-data/nodeengine/world.json --smoke-test .verification-data/nodeengine/scene.png
```

The graphics smoke run opens a window for 60 frames, exercises block-edit/remesh/save, writes the screenshot, and exits. Use a disposable test world: it adds a block.

Verified on Windows during this implementation: all 13 engine tests passed; Clippy passed with warnings denied; the optimized executable completed the 60-frame textured graphics run and reloaded its saved edit. CLI export/import/export also succeeded. The generated reference screenshot is `.verification-data/nodeengine-release/scene.png` (local, ignored by Git).

## Compatibility roadmap

1. Completed first milestone: native sandbox, terrain, meshing, movement, edits, persistence, and a small schematic palette.
2. Expand the block-state registry and add read-only Anvil/NBT chunk import with fixture tests. Keep original Minecraft worlds untouched.
3. Choose one specific Java Edition protocol version, implement its handshake/login/configuration/play states, and connect NodeClient's verified startup session without exposing tokens to the renderer.
4. Add compatible inventory, crafting, entities, lighting, fluids, and world-generation behavior incrementally with reference fixtures.
5. Integrate engine selection and verified native releases into NodeClient only after the selected compatibility target passes end-to-end testing.

Minecraft servers, account login, vanilla world loading, survival rules, mobs, redstone, and Fabric/Forge/NeoForge Java mods do **not** work in this milestone. Standalone NodeEngine currently uses only its original local sandbox and has no Microsoft or Minecraft Services network access.
