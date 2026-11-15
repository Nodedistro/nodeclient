use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::{
    fs::File,
    io::{Read, Write},
    path::Path,
};

pub const SIZE: [i32; 3] = [64, 48, 64];
pub const VOLUME: usize = (SIZE[0] * SIZE[1] * SIZE[2]) as usize;
pub const CHUNK: i32 = 16;
pub const BLOCK_NAMES: [&str; 8] = [
    "minecraft:air",
    "minecraft:grass_block[snowy=false]",
    "minecraft:dirt",
    "minecraft:stone",
    "minecraft:sand",
    "minecraft:oak_log[axis=y]",
    "minecraft:oak_leaves[distance=7,persistent=true,waterlogged=false]",
    "minecraft:oak_planks",
];

#[derive(Clone)]
pub struct World {
    pub seed: u32,
    blocks: Vec<u8>,
    pub revision: u64,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Save {
    format: String,
    version: u32,
    seed: u32,
    size: [i32; 3],
    blocks: Vec<u8>,
}

impl World {
    pub fn empty(seed: u32) -> Self {
        Self {
            seed,
            blocks: vec![0; VOLUME],
            revision: 0,
        }
    }
    pub fn contains(p: [i32; 3]) -> bool {
        (0..3).all(|i| p[i] >= 0 && p[i] < SIZE[i])
    }
    fn index([x, y, z]: [i32; 3]) -> usize {
        (x + z * SIZE[0] + y * SIZE[0] * SIZE[2]) as usize
    }
    pub fn get(&self, p: [i32; 3]) -> u8 {
        if Self::contains(p) {
            self.blocks[Self::index(p)]
        } else {
            0
        }
    }
    pub fn set(&mut self, p: [i32; 3], block: u8) -> bool {
        if !Self::contains(p) || block as usize >= BLOCK_NAMES.len() {
            return false;
        }
        let index = Self::index(p);
        if self.blocks[index] == block {
            return false;
        }
        self.blocks[index] = block;
        self.revision += 1;
        true
    }
    pub fn solid(&self, p: [i32; 3]) -> bool {
        self.get(p) != 0
    }
    pub fn generate(seed: u32) -> Self {
        let mut world = Self::empty(seed);
        for z in 0..SIZE[2] {
            for x in 0..SIZE[0] {
                let height = (10.0
                    + noise(x as f32 / 18.0, z as f32 / 18.0, seed) * 10.0
                    + noise(x as f32 / 7.0, z as f32 / 7.0, seed ^ 83) * 3.0)
                    as i32;
                for y in 0..=height {
                    world.set(
                        [x, y, z],
                        if y == height {
                            1
                        } else if y > height - 3 {
                            2
                        } else {
                            3
                        },
                    );
                }
            }
        }
        for z in (5..SIZE[2] - 5).step_by(9) {
            for x in (5..SIZE[0] - 5).step_by(9) {
                if hash(x, z, seed).is_multiple_of(3) || (x - 32).abs() + (z - 32).abs() < 10 {
                    continue;
                }
                let ground = world.top(x, z);
                for y in ground + 1..=ground + 5 {
                    world.set([x, y, z], 5);
                }
                for dy in 3..=6 {
                    for dz in -2i32..=2 {
                        for dx in -2i32..=2 {
                            if dx.abs() + dz.abs() > if dy == 6 { 1 } else { 3 } {
                                continue;
                            }
                            let p = [x + dx, ground + dy, z + dz];
                            if world.get(p) == 0 {
                                world.set(p, 6);
                            }
                        }
                    }
                }
            }
        }
        world
    }
    pub fn top(&self, x: i32, z: i32) -> i32 {
        (0..SIZE[1])
            .rev()
            .find(|&y| self.solid([x, y, z]))
            .unwrap_or(0)
    }
    pub fn spawn(&self) -> glam::Vec3 {
        // Find headroom, including in imported structures.
        for radius in 0..32i32 {
            for z in 32 - radius..=32 + radius {
                for x in 32 - radius..=32 + radius {
                    if x < 1 || z < 1 || x >= SIZE[0] - 1 || z >= SIZE[2] - 1 {
                        continue;
                    }
                    let y = self.top(x, z) + 1;
                    if y + 2 < SIZE[1] {
                        return glam::vec3(x as f32 + 0.5, y as f32 + 0.01, z as f32 + 0.5);
                    }
                }
            }
        }
        glam::vec3(32.5, SIZE[1] as f32 + 2.0, 32.5)
    }
    pub fn save(&self, path: &Path) -> Result<()> {
        let parent = path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        std::fs::create_dir_all(parent)?;
        let mut file = tempfile::NamedTempFile::new_in(parent)?;
        serde_json::to_writer(
            &mut file,
            &Save {
                format: "nodeengine-world".into(),
                version: 1,
                seed: self.seed,
                size: SIZE,
                blocks: self.blocks.clone(),
            },
        )?;
        file.flush()?;
        file.as_file().sync_all()?;
        file.persist(path)
            .map_err(|e| e.error)
            .context("Could not replace world save")?;
        Ok(())
    }
    pub fn load(path: &Path) -> Result<Self> {
        let mut bytes = Vec::new();
        File::open(path)?.take(2_000_001).read_to_end(&mut bytes)?;
        if bytes.len() > 2_000_000 {
            bail!("World save exceeds 2 MB");
        }
        let save: Save = serde_json::from_slice(&bytes)
            .context("Invalid NodeEngine world; original file was not changed")?;
        if save.format != "nodeengine-world"
            || save.version != 1
            || save.size != SIZE
            || save.blocks.len() != VOLUME
            || save.blocks.iter().any(|&b| b as usize >= BLOCK_NAMES.len())
        {
            bail!("Unsupported or corrupt NodeEngine world");
        }
        Ok(Self {
            seed: save.seed,
            blocks: save.blocks,
            revision: 0,
        })
    }
}
fn hash(x: i32, z: i32, seed: u32) -> u32 {
    let mut h = (x as u32).wrapping_mul(374761393) ^ (z as u32).wrapping_mul(668265263) ^ seed;
    h = (h ^ (h >> 13)).wrapping_mul(1274126177);
    h ^ (h >> 16)
}
fn noise(x: f32, z: f32, seed: u32) -> f32 {
    let ix = x.floor() as i32;
    let iz = z.floor() as i32;
    let smooth = |v: f32| v * v * (3.0 - 2.0 * v);
    let tx = smooth(x - x.floor());
    let tz = smooth(z - z.floor());
    let value = |x, z| (hash(x, z, seed) & 65535) as f32 / 65535.0;
    let a = value(ix, iz) * (1.0 - tx) + value(ix + 1, iz) * tx;
    let b = value(ix, iz + 1) * (1.0 - tx) + value(ix + 1, iz + 1) * tx;
    a * (1.0 - tz) + b * tz
}
