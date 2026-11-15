//! Sponge schematic v2 block-volume interchange, not Minecraft world-save support.
use crate::world::{World, BLOCK_NAMES, SIZE};
use anyhow::{bail, Context, Result};
use fastnbt::{ByteArray, IntArray};
use flate2::{read::GzDecoder, write::GzEncoder, Compression};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    fs::File,
    io::{Read, Write},
    path::Path,
};

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct Schematic {
    version: i32,
    data_version: i32,
    width: i16,
    height: i16,
    length: i16,
    #[serde(default)]
    offset: Option<IntArray>,
    palette_max: i32,
    palette: HashMap<String, i32>,
    block_data: ByteArray,
    #[serde(default)]
    block_entities: Vec<HashMap<String, fastnbt::Value>>,
    #[serde(default)]
    entities: Vec<HashMap<String, fastnbt::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    biome_data: Option<ByteArray>,
}
/// Imports a supported block volume centered in a fresh world, without changing its source.
pub fn import(path: &Path) -> Result<World> {
    let mut bytes = Vec::new();
    GzDecoder::new(File::open(path)?)
        .take(8_000_001)
        .read_to_end(&mut bytes)?;
    if bytes.len() > 8_000_000 {
        bail!("Schematic exceeds the 8 MB decompressed limit");
    }
    let source: Schematic =
        fastnbt::from_bytes(&bytes).context("Expected gzip-compressed Sponge schematic v2")?;
    decode(source)
}
fn decode(s: Schematic) -> Result<World> {
    if s.version != 2 {
        bail!("Only Sponge schematic version 2 is supported");
    }
    let size = [
        s.width as u16 as i32,
        s.height as u16 as i32,
        s.length as u16 as i32,
    ];
    if (0..3).any(|i| size[i] <= 0 || size[i] > SIZE[i]) {
        bail!("Structure must fit within 64 x 48 x 64 blocks");
    }
    if !s.entities.is_empty() || !s.block_entities.is_empty() || s.biome_data.is_some() {
        bail!("This milestone imports blocks only; entities, block entities, and biome data are unsupported");
    }
    if s.offset.as_ref().is_some_and(|v| v.len() != 3) {
        bail!("Invalid schematic offset");
    }
    if s.palette_max <= 0 || s.palette_max > 65536 {
        bail!("Invalid schematic palette size");
    }
    let mut palette = HashMap::new();
    for (name, id) in s.palette {
        if id < 0 || id >= s.palette_max || palette.contains_key(&id) {
            bail!("Invalid or duplicate palette index");
        }
        let block = BLOCK_NAMES
            .iter()
            .position(|&n| n == name)
            .or(match name.as_str() {
                "minecraft:grass_block" => Some(1),
                "minecraft:oak_log" => Some(5),
                "minecraft:oak_leaves" => Some(6),
                _ => None,
            })
            .with_context(|| {
                format!("Unsupported block state: {name}. The source file has not been changed.")
            })?;
        palette.insert(id, block as u8);
    }
    let mut world = World::empty(0);
    let mut cursor = 0;
    let ox = (SIZE[0] - size[0]) / 2;
    let oz = (SIZE[2] - size[2]) / 2;
    for y in 0..size[1] {
        for z in 0..size[2] {
            for x in 0..size[0] {
                let id = varint(&s.block_data, &mut cursor)?;
                let block = *palette
                    .get(&id)
                    .context("Block data references an absent palette entry")?;
                world.set([x + ox, y, z + oz], block);
            }
        }
    }
    if cursor != s.block_data.len() {
        bail!("Unexpected trailing block data");
    }
    Ok(world)
}
fn varint(data: &[i8], cursor: &mut usize) -> Result<i32> {
    let mut result = 0u32;
    for shift in (0..35).step_by(7) {
        let byte = *data
            .get(*cursor)
            .context("Truncated schematic block data")? as u8;
        *cursor += 1;
        if shift == 28 && byte > 7 {
            bail!("Overflowing palette varint");
        }
        result |= ((byte & 127) as u32) << shift;
        if byte & 128 == 0 {
            return Ok(result as i32);
        }
    }
    bail!("Invalid palette varint")
}
pub fn export(world: &World, path: &Path) -> Result<()> {
    let mut data = Vec::new();
    for y in 0..SIZE[1] {
        for z in 0..SIZE[2] {
            for x in 0..SIZE[0] {
                data.push(world.get([x, y, z]) as i8);
            }
        }
    }
    let schematic = Schematic {
        version: 2,
        data_version: 3465,
        width: SIZE[0] as i16,
        height: SIZE[1] as i16,
        length: SIZE[2] as i16,
        offset: Some(IntArray::new(vec![0, 0, 0])),
        palette_max: BLOCK_NAMES.len() as i32,
        palette: BLOCK_NAMES
            .iter()
            .enumerate()
            .map(|(i, s)| (s.to_string(), i as i32))
            .collect(),
        block_data: ByteArray::new(data),
        block_entities: vec![],
        entities: vec![],
        biome_data: None,
    };
    let bytes =
        fastnbt::to_bytes_with_opts(&schematic, fastnbt::SerOpts::new().root_name("Schematic"))?;
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    std::fs::create_dir_all(parent)?;
    let mut file = tempfile::NamedTempFile::new_in(parent)?;
    {
        let mut encoder = GzEncoder::new(&mut file, Compression::default());
        encoder.write_all(&bytes)?;
        encoder.finish()?;
    }
    file.as_file().sync_all()?;
    file.persist_noclobber(path)
        .map_err(|e| e.error)
        .context("Export destination already exists or is not writable")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn sample() -> Schematic {
        Schematic {
            version: 2,
            data_version: 3465,
            width: 1,
            height: 1,
            length: 1,
            offset: None,
            palette_max: 1,
            palette: HashMap::from([("minecraft:stone".into(), 0)]),
            block_data: ByteArray::new(vec![0]),
            entities: vec![],
            block_entities: vec![],
            biome_data: None,
        }
    }
    #[test]
    fn unsupported_content_is_rejected_instead_of_substituted() {
        let mut source = sample();
        source.palette = HashMap::from([("minecraft:chest[facing=north]".into(), 0)]);
        assert!(decode(source)
            .err()
            .unwrap()
            .to_string()
            .contains("Unsupported block state"));
        let mut source = sample();
        source.entities.push(HashMap::new());
        assert!(decode(source).is_err());
        let mut source = sample();
        source.version = 3;
        assert!(decode(source).is_err());
    }
    #[test]
    fn invalid_dimensions_and_palette_references_fail() {
        let mut source = sample();
        source.width = 65;
        assert!(decode(source).is_err());
        let mut source = sample();
        source.width = -1;
        assert!(decode(source).is_err());
        let mut source = sample();
        source.block_data = ByteArray::new(vec![1]);
        assert!(decode(source).is_err());
        let mut source = sample();
        source.block_data = ByteArray::new(vec![0, 0]);
        assert!(decode(source).is_err());
    }
    #[test]
    fn decodes_multibyte_palette_ids_and_rejects_truncation() {
        assert_eq!(varint(&[-128, 1], &mut 0).unwrap(), 128);
        assert!(varint(&[-128], &mut 0).is_err());
        assert!(varint(&[-1, -1, -1, -1, 127], &mut 0).is_err());
    }
    #[test]
    fn schematic_roundtrip_preserves_coordinates_and_blocks() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.schem");
        let mut world = World::empty(0);
        world.set([1, 2, 3], 7);
        world.set([63, 47, 63], 5);
        export(&world, &path).unwrap();
        let imported = import(&path).unwrap();
        assert_eq!(imported.get([1, 2, 3]), 7);
        assert_eq!(imported.get([63, 47, 63]), 5);
        assert_eq!(imported.get([3, 2, 1]), 0);
        assert!(export(&world, &path).is_err());
    }
    #[test]
    fn external_palette_order_and_dimensions_are_respected() {
        let source = Schematic {
            version: 2,
            data_version: 3465,
            width: 2,
            height: 1,
            length: 1,
            offset: None,
            palette_max: 129,
            palette: HashMap::from([("minecraft:stone".into(), 128), ("minecraft:air".into(), 0)]),
            block_data: ByteArray::new(vec![-128, 1, 0]),
            entities: vec![],
            block_entities: vec![],
            biome_data: None,
        };
        let world = decode(source).unwrap();
        assert_eq!(world.get([31, 0, 31]), 3);
        assert_eq!(world.get([32, 0, 31]), 0);
    }
}
