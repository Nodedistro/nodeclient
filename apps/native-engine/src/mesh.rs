use crate::world::{World, CHUNK, SIZE};
/// Each chunk emits only faces bordering air, including across chunk boundaries.
#[derive(Clone)]
pub struct Face {
    pub corners: [[f32; 3]; 4],
    pub block: u8,
    pub tile: u8,
    pub shade: f32,
}
const NORMALS: [[i32; 3]; 6] = [
    [1, 0, 0],
    [-1, 0, 0],
    [0, 1, 0],
    [0, -1, 0],
    [0, 0, 1],
    [0, 0, -1],
];
const CORNERS: [[[f32; 3]; 4]; 6] = [
    [[1., 0., 0.], [1., 1., 0.], [1., 1., 1.], [1., 0., 1.]],
    [[0., 0., 1.], [0., 1., 1.], [0., 1., 0.], [0., 0., 0.]],
    [[0., 1., 1.], [1., 1., 1.], [1., 1., 0.], [0., 1., 0.]],
    [[0., 0., 0.], [1., 0., 0.], [1., 0., 1.], [0., 0., 1.]],
    [[1., 0., 1.], [1., 1., 1.], [0., 1., 1.], [0., 0., 1.]],
    [[0., 0., 0.], [0., 1., 0.], [1., 1., 0.], [1., 0., 0.]],
];
pub fn chunk_faces(world: &World, cx: i32, cz: i32) -> Vec<Face> {
    let mut faces = Vec::new();
    for y in 0..SIZE[1] {
        for z in cz * CHUNK..(cz + 1) * CHUNK {
            for x in cx * CHUNK..(cx + 1) * CHUNK {
                let block = world.get([x, y, z]);
                if block == 0 {
                    continue;
                }
                for (side, n) in NORMALS.iter().enumerate() {
                    if !world.solid([x + n[0], y + n[1], z + n[2]]) {
                        let tint = 1.0 - ((x * 13 + z * 7 + y * 3).rem_euclid(7) as f32) * 0.009;
                        faces.push(Face {
                            corners: CORNERS[side]
                                .map(|p| [p[0] + x as f32, p[1] + y as f32, p[2] + z as f32]),
                            block,
                            tile: match (block, side) {
                                (1, 2) => 1,
                                (1, 3) => 2,
                                (1, _) => 8,
                                (5, 2 | 3) => 9,
                                _ => block,
                            },
                            shade: [0.80, 0.65, 1.0, 0.45, 0.88, 0.72][side] * tint,
                        });
                    }
                }
            }
        }
    }
    faces
}
