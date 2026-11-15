use glam::{vec3, Vec3};
use nodeengine::{
    mesh::chunk_faces,
    physics::{collides, raycast, Player},
    world::{World, SIZE},
};

fn floor() -> World {
    let mut world = World::empty(0);
    for x in 0..SIZE[0] {
        for z in 0..SIZE[2] {
            world.set([x, 0, z], 3);
        }
    }
    world
}
#[test]
fn seeds_are_reproducible_and_distinct() {
    let a = World::generate(12);
    let b = World::generate(12);
    let c = World::generate(13);
    let mut different = false;
    for x in 0..64 {
        for z in 0..64 {
            assert_eq!(a.top(x, z), b.top(x, z));
            different |= a.top(x, z) != c.top(x, z);
        }
    }
    assert!(different);
    assert!(!collides(&a, a.spawn()));
}
#[test]
fn saves_survive_edits_and_invalid_saves_are_not_replaced() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("world.json");
    let mut world = World::generate(12);
    world.set([10, 30, 20], 7);
    world.save(&path).unwrap();
    world.set([10, 30, 20], 0);
    world.save(&path).unwrap();
    let restored = World::load(&path).unwrap();
    assert_eq!(restored.get([10, 30, 20]), 0);
    assert_eq!(restored.seed, 12);
    std::fs::write(&path, "invalid").unwrap();
    assert!(World::load(&path).is_err());
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "invalid");
}
#[test]
fn rejects_outside_coordinates_and_unknown_blocks() {
    let mut world = World::empty(0);
    assert!(!world.set([-1, 0, 0], 1));
    assert!(!world.set([64, 0, 0], 1));
    assert!(!world.set([0, 48, 0], 1));
    assert!(!world.set([0, 0, 0], 255));
    assert_eq!(world.get([-1, 0, 0]), 0);
}
#[test]
fn meshing_culls_interior_faces_across_chunk_boundaries() {
    let mut world = World::empty(0);
    world.set([15, 3, 5], 3);
    assert_eq!(chunk_faces(&world, 0, 0).len(), 6);
    world.set([16, 3, 5], 3);
    assert_eq!(chunk_faces(&world, 0, 0).len(), 5);
    assert_eq!(chunk_faces(&world, 1, 0).len(), 5);
}
#[test]
fn raycast_respects_reach_and_returns_placement_neighbor() {
    let mut world = World::empty(0);
    world.set([3, 2, 2], 3);
    let hit = raycast(&world, vec3(1.5, 2.5, 2.5), Vec3::X, 6.).unwrap();
    assert_eq!(hit.block, [3, 2, 2]);
    assert_eq!(hit.adjacent, [2, 2, 2]);
    assert!(raycast(&world, vec3(1.5, 2.5, 2.5), Vec3::X, 1.).is_none());
    assert_eq!(
        raycast(&world, vec3(5.5, 2.5, 2.5), -Vec3::X, 6.)
            .unwrap()
            .adjacent,
        [4, 2, 2]
    );
    assert!(raycast(&world, Vec3::ZERO, Vec3::ZERO, 6.).is_none());
    assert!(raycast(&world, Vec3::NAN, Vec3::X, 6.).is_none());
}
#[test]
fn gravity_lands_without_tunneling_and_jump_clears_floor() {
    let world = floor();
    let mut player = Player::new(vec3(10.5, 30., 10.5));
    for _ in 0..500 {
        player.tick(&world, Vec3::ZERO, false, false);
    }
    assert!(player.grounded);
    assert!((player.position.y - 1.).abs() < 0.001);
    assert!(!collides(&world, player.position));
    player.tick(&world, Vec3::ZERO, true, false);
    assert!(player.velocity.y > 0.);
    assert!(player.position.y > 1.);
}
#[test]
fn walls_stop_players_and_placement_detects_body_overlap() {
    let mut world = floor();
    for y in 1..5 {
        for z in 0..64 {
            world.set([12, y, z], 3);
        }
    }
    let mut player = Player::new(vec3(10.5, 1., 10.5));
    for _ in 0..500 {
        player.tick(&world, Vec3::X, false, true);
    }
    assert!(player.position.x <= 11.701);
    assert!(!collides(&world, player.position));
    assert!(player.overlaps([11, 1, 10]));
    assert!(!player.overlaps([14, 1, 10]));
}
#[test]
fn diagonal_motion_is_not_faster() {
    let world = floor();
    let start = vec3(20.5, 1., 20.5);
    let mut a = Player::new(start);
    let mut b = Player::new(start);
    for _ in 0..100 {
        a.tick(&world, Vec3::X, false, false);
        b.tick(&world, Vec3::X + Vec3::Z, false, false);
    }
    assert!(((a.position - start).length() - (b.position - start).length()).abs() < 0.001);
}
