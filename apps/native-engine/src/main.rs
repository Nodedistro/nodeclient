use anyhow::{bail, Context, Result};
mod textures;
use macroquad::prelude::*;
use nodeengine::{
    mesh::chunk_faces,
    physics::{raycast, Player, STEP},
    schematic,
    world::{World, CHUNK, SIZE},
};
use std::{collections::HashSet, path::PathBuf};

const COLORS: [Color; 8] = [
    BLANK,
    Color::new(0.43, 0.67, 0.25, 1.),
    Color::new(0.50, 0.32, 0.19, 1.),
    Color::new(0.56, 0.59, 0.61, 1.),
    Color::new(0.86, 0.78, 0.53, 1.),
    Color::new(0.40, 0.27, 0.14, 1.),
    Color::new(0.22, 0.48, 0.25, 1.),
    Color::new(0.68, 0.49, 0.28, 1.),
];
const LABELS: [&str; 8] = [
    "Air", "Grass", "Dirt", "Stone", "Sand", "Oak log", "Leaves", "Planks",
];
struct Options {
    world: PathBuf,
    seed: u32,
    import: Option<PathBuf>,
    export: Option<PathBuf>,
    smoke: Option<PathBuf>,
}
fn options() -> Result<Option<Options>> {
    let base = std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    let mut opts = Options {
        world: base.join("Nodedistro/NodeEngine/worlds/first-world.json"),
        seed: 42,
        import: None,
        export: None,
        smoke: None,
    };
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        if arg == "--help" || arg == "-h" {
            println!("NodeEngine 0.1 — Nodedistro native voxel prototype\n\n--world PATH          Own world save (never a Minecraft save)\n--seed NUMBER         Seed for a new world (default 42)\n--import-schematic P  Import supported Sponge v2 blocks into a NEW --world\n--export-schematic P  Export blocks to a NEW .schem file and exit\n--smoke-test PNG      Render 60 frames, write a screenshot, and exit\n\nWASD move; mouse look; Space jump; Shift sprint; 1–7 blocks\nLeft click break; right click place; Escape pause; F5 save; Q quit while paused");
            return Ok(None);
        }
        let value = args
            .next()
            .with_context(|| format!("Missing value for {arg}"))?;
        match arg.as_str() {
            "--world" => opts.world = value.into(),
            "--seed" => {
                opts.seed = value
                    .parse()
                    .context("Seed must be a positive 32-bit integer")?
            }
            "--import-schematic" => opts.import = Some(value.into()),
            "--export-schematic" => opts.export = Some(value.into()),
            "--smoke-test" => opts.smoke = Some(value.into()),
            _ => bail!("Unknown option {arg}; use --help"),
        }
    }
    Ok(Some(opts))
}
fn main() {
    if let Err(e) = start() {
        eprintln!("NodeEngine: {e:#}");
        std::process::exit(1);
    }
}
fn start() -> Result<()> {
    let Some(opts) = options()? else {
        return Ok(());
    };
    if opts.world.extension().and_then(|s| s.to_str()) != Some("json") {
        bail!("Use a .json NodeEngine world path, separate from Minecraft worlds");
    }
    let parent = opts
        .world
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(std::path::Path::new("."));
    std::fs::create_dir_all(parent)?;
    let lock = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(opts.world.with_extension("lock"))?;
    fs2::FileExt::try_lock_exclusive(&lock)
        .context("This world is already open in another NodeEngine process")?;
    let world = if let Some(path) = &opts.import {
        if opts.world.exists() {
            bail!("Import needs a new world path; existing world was not changed");
        }
        schematic::import(path)?
    } else if opts.world.exists() {
        World::load(&opts.world)?
    } else {
        World::generate(opts.seed)
    };
    if let Some(path) = &opts.export {
        schematic::export(&world, path)?;
        println!("Exported {}", path.display());
        return Ok(());
    }
    world.save(&opts.world)?;
    if let Some(path) = &opts.smoke {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
    }
    let config = Conf {
        window_title: "NodeEngine — Nodedistro | Native Rust prototype".into(),
        window_width: 1280,
        window_height: 800,
        sample_count: 4,
        ..Default::default()
    };
    macroquad::Window::from_config(config, run(world, opts, lock));
    Ok(())
}
fn build_meshes(world: &World, cx: i32, cz: i32, atlas: &Texture2D) -> Vec<Mesh> {
    let faces = chunk_faces(world, cx, cz);
    let mut meshes = Vec::new();
    // Stay within Macroquad's default per-draw vertex/index capacities and u16 indices.
    for group in faces.chunks(750) {
        let mut mesh = Mesh {
            vertices: Vec::with_capacity(group.len() * 4),
            indices: Vec::with_capacity(group.len() * 6),
            texture: Some(atlas.clone()),
        };
        for face in group {
            let base = mesh.vertices.len() as u16;
            let shade = Color::new(face.shade, face.shade, face.shade, 1.);
            for (p, [u, v]) in
                face.corners
                    .into_iter()
                    .zip([[0., 1.], [0., 0.], [1., 0.], [1., 1.]])
            {
                let u = (face.tile as f32 * 16.0 + 0.1 + u * 15.8) / 160.0;
                let v = (0.1 + v * 15.8) / 16.0;
                mesh.vertices
                    .push(Vertex::new(p[0], p[1], p[2], u, v, shade));
            }
            mesh.indices
                .extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
        }
        meshes.push(mesh);
    }
    meshes
}
fn dirty_chunks(p: [i32; 3]) -> HashSet<(i32, i32)> {
    let mut chunks = HashSet::new();
    for (dx, dz) in [(0, 0), (-1, 0), (1, 0), (0, -1), (0, 1)] {
        let x = p[0] + dx;
        let z = p[2] + dz;
        if x >= 0 && x < SIZE[0] && z >= 0 && z < SIZE[2] {
            chunks.insert((x / CHUNK, z / CHUNK));
        }
    }
    chunks
}
async fn run(mut world: World, opts: Options, _lock: std::fs::File) {
    let atlas = textures::atlas();
    let mut chunks: Vec<Vec<Mesh>> = (0..SIZE[2] / CHUNK)
        .flat_map(|z| (0..SIZE[0] / CHUNK).map(move |x| (x, z)))
        .map(|(x, z)| build_meshes(&world, x, z, &atlas))
        .collect();
    let mut player = Player::new(world.spawn());
    let mut yaw = 0.55f32;
    let mut pitch = -0.18f32;
    let mut paused = opts.smoke.is_none();
    let mut selected = 3u8;
    let mut accumulator = 0f32;
    let mut note = "World saved locally".to_string();
    let mut saved_revision = world.revision;
    let mut frames = 0;
    let mut smoke_action_done = false;
    loop {
        frames += 1;
        let mut just_resumed = false;
        if is_key_pressed(KeyCode::Escape) || (paused && is_mouse_button_pressed(MouseButton::Left))
        {
            paused = !paused;
            set_cursor_grab(!paused);
            show_mouse(paused);
            accumulator = 0.;
            just_resumed = true;
        }
        if paused && is_key_pressed(KeyCode::Q) {
            match world.save(&opts.world) {
                Ok(()) => break,
                Err(e) => note = format!("Save failed: {e}. F5 retries; your world is still open."),
            }
        }
        for (i, key) in [
            KeyCode::Key1,
            KeyCode::Key2,
            KeyCode::Key3,
            KeyCode::Key4,
            KeyCode::Key5,
            KeyCode::Key6,
            KeyCode::Key7,
        ]
        .iter()
        .enumerate()
        {
            if is_key_pressed(*key) {
                selected = i as u8 + 1;
            }
        }
        if !paused && !just_resumed && opts.smoke.is_none() {
            let delta = mouse_delta_position();
            yaw += delta.x * 2.0;
            pitch = (pitch + delta.y * 2.0).clamp(-1.54, 1.54);
        }
        let direction = ::glam::vec3(
            yaw.sin() * pitch.cos(),
            pitch.sin(),
            yaw.cos() * pitch.cos(),
        );
        if !paused {
            let forward = ::glam::vec3(yaw.sin(), 0., yaw.cos());
            let right = ::glam::vec3(-yaw.cos(), 0., yaw.sin());
            let mut movement = ::glam::Vec3::ZERO;
            if is_key_down(KeyCode::W) {
                movement += forward;
            }
            if is_key_down(KeyCode::S) {
                movement -= forward;
            }
            if is_key_down(KeyCode::D) {
                movement += right;
            }
            if is_key_down(KeyCode::A) {
                movement -= right;
            }
            accumulator += get_frame_time().min(0.1);
            while accumulator >= STEP {
                player.tick(
                    &world,
                    movement,
                    is_key_down(KeyCode::Space),
                    is_key_down(KeyCode::LeftShift),
                );
                accumulator -= STEP;
            }
        }
        let hit = raycast(&world, player.eye(), direction, 6.0);
        let mut edited = None;
        if !paused && !just_resumed {
            if let Some(hit) = &hit {
                if is_mouse_button_pressed(MouseButton::Left)
                    && hit.block[1] > 0
                    && world.set(hit.block, 0)
                {
                    edited = Some(hit.block);
                }
                if is_mouse_button_pressed(MouseButton::Right)
                    && !player.overlaps(hit.adjacent)
                    && !world.solid(hit.adjacent)
                    && world.set(hit.adjacent, selected)
                {
                    edited = Some(hit.adjacent);
                }
            }
        }
        // Exercise the production edit/remesh/save path during automated render verification.
        if opts.smoke.is_some() && frames == 15 {
            let p = [34, world.top(34, 34) + 1, 34];
            if world.set(p, 7) {
                edited = Some(p);
                smoke_action_done = true;
            }
        }
        if let Some(p) = edited {
            for (cx, cz) in dirty_chunks(p) {
                chunks[(cx + cz * (SIZE[0] / CHUNK)) as usize] =
                    build_meshes(&world, cx, cz, &atlas);
            }
        }
        if edited.is_some() || is_key_pressed(KeyCode::F5) {
            match world.save(&opts.world) {
                Ok(()) => {
                    note = "World saved locally".into();
                    saved_revision = world.revision;
                }
                Err(e) => note = format!("Save failed: {e}. Press F5 to retry."),
            }
        }
        clear_background(Color::from_rgba(156, 201, 218, 255));
        let eye = player.eye();
        let target = eye + direction;
        set_camera(&Camera3D {
            position: vec3(eye.x, eye.y, eye.z),
            target: vec3(target.x, target.y, target.z),
            up: Vec3::Y,
            fovy: 70.0f32.to_radians(),
            z_near: 0.05,
            z_far: 180.,
            ..Default::default()
        });
        for chunk in &chunks {
            for mesh in chunk {
                draw_mesh(mesh);
            }
        }
        if let Some(hit) = &hit {
            let [x, y, z] = hit.block;
            draw_cube_wires(
                vec3(x as f32 + 0.5, y as f32 + 0.5, z as f32 + 0.5),
                vec3(1.006, 1.006, 1.006),
                WHITE,
            );
        }
        set_default_camera();
        hud(
            &player,
            selected,
            paused,
            &note,
            saved_revision == world.revision,
        );
        if let Some(path) = opts.smoke.as_ref().filter(|_| frames >= 60) {
            assert!(smoke_action_done, "Smoke test failed to edit a block");
            assert_eq!(saved_revision, world.revision, "Smoke test failed to save");
            let reloaded = World::load(&opts.world).expect("Smoke save must be readable");
            assert_eq!(reloaded.get([34, world.top(34, 34), 34]), 7);
            get_screen_data().export_png(path.to_str().unwrap_or("nodeengine-smoke.png"));
            println!("NodeEngine render smoke: {frames} frames, {} draw meshes, edit={}, save={}, screenshot={}",chunks.iter().map(Vec::len).sum::<usize>(),smoke_action_done,saved_revision==world.revision,path.display());
            break;
        }
        next_frame().await;
    }
    set_cursor_grab(false);
    show_mouse(true);
}
fn hud(player: &Player, selected: u8, paused: bool, note: &str, saved: bool) {
    let w = screen_width();
    let h = screen_height();
    draw_rectangle(20., 20., 310., 78., Color::new(0.035, 0.07, 0.08, 0.88));
    draw_text("NODEENGINE", 36., 51., 27., WHITE);
    draw_text(
        "NODEDISTRO  /  NATIVE RUST  /  0.1",
        36.,
        78.,
        16.,
        Color::from_rgba(169, 206, 183, 255),
    );
    let stats = format!(
        "{} FPS   |   {:.0}, {:.0}, {:.0}",
        get_fps(),
        player.position.x,
        player.position.y,
        player.position.z
    );
    draw_text(
        &stats,
        w - 285.,
        40.,
        19.,
        Color::from_rgba(29, 49, 53, 255),
    );
    draw_line(w / 2. - 6., h / 2., w / 2. + 6., h / 2., 2., WHITE);
    draw_line(w / 2., h / 2. - 6., w / 2., h / 2. + 6., 2., WHITE);
    let start = w / 2. - 7. * 60. / 2.;
    for i in 1..=7 {
        let x = start + (i - 1) as f32 * 60.;
        draw_rectangle(x, h - 96., 54., 56., Color::new(0.035, 0.07, 0.08, 0.86));
        if selected == i {
            draw_rectangle_lines(
                x - 2.,
                h - 98.,
                58.,
                60.,
                3.,
                Color::from_rgba(182, 231, 160, 255),
            );
        }
        draw_rectangle(x + 14., h - 85., 26., 24., COLORS[i as usize]);
        draw_text(i.to_string(), x + 6., h - 44., 14., WHITE);
    }
    let name = LABELS[selected as usize];
    let width = measure_text(name, None, 20, 1.0).width;
    draw_text(name, w / 2. - width / 2., h - 110., 20., WHITE);
    draw_text(note, 20., h - 16., 17., if saved { WHITE } else { YELLOW });
    if paused {
        let x = (w - 500.) / 2.;
        let y = (h - 230.) / 2.;
        draw_rectangle(x, y, 500., 230., Color::new(0.035, 0.07, 0.08, 0.94));
        draw_text("YOUR WORLD. NATIVE RUST.", x + 28., y + 42., 27., WHITE);
        draw_text(
            "Click to play",
            x + 28.,
            y + 82.,
            25.,
            Color::from_rgba(182, 231, 160, 255),
        );
        draw_text(
            "WASD move   /   Mouse look   /   Space jump",
            x + 28.,
            y + 119.,
            19.,
            WHITE,
        );
        draw_text(
            "Left click break   /   Right click build   /   1-7 blocks",
            x + 28.,
            y + 148.,
            18.,
            WHITE,
        );
        draw_text(
            "Shift sprint   /   Esc pause   /   Q save and quit",
            x + 28.,
            y + 177.,
            18.,
            WHITE,
        );
        draw_text(
            "Prototype: local sandbox + limited .schem interchange",
            x + 28.,
            y + 207.,
            16.,
            GRAY,
        );
    }
}
