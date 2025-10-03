use bevy::{
    camera::primitives::Aabb,
    math::bounding::{Aabb2d, BoundingVolume},
    picking::pointer::{PointerId, PointerLocation},
    prelude::*,
    sprite::Text2dShadow,
    window::PrimaryWindow,
};

use crate::pancam::{NewScale, SmoothZoom};
use miniproj::get_projection;
use tilemath::{BBox, bbox_covered_tiles};

mod pancam;
use pancam::pancam_plugin;

const TILE_SIZE: f32 = 256.;

pub struct MapPlugin;

impl Plugin for MapPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(pancam_plugin)
            .add_systems(Startup, (setup))
            .add_systems(PostStartup, move_cam_once)
            .add_systems(Update, debug_draw)
            .add_observer(handle_zoom_level);
    }
}

#[derive(Component, Copy, Clone)]
struct Tile;

fn setup(mut commands: Commands, asset_server: Res<AssetServer>) {
    let proj = get_projection(3857).unwrap();
    let (max_x, max_y) = proj.deg_to_projected(14.261350595825427, 54.963283243087496);
    let (min_x, min_y) = proj.deg_to_projected(5.5437553459687186, 46.2960593038139);
    let bbox = BBox {
        min_x,
        min_y,
        max_x,
        max_y,
    };
    let zoom = 9;
    let tiles = bbox_covered_tiles(&bbox, zoom).collect::<Vec<_>>();
    info!("Tiles to load: {:?}", tiles);
    for tile in tiles {
        info!("Loading tile: {:?}", tile);
        let tms_tile = tile.to_reversed_y();
        let url = format!(
            "https://mapproxy.dmho.de/tms/1.0.0/thunderforest_transport/EPSG3857/{}/{}/{}.png",
            tms_tile.zoom - 1,
            tms_tile.x,
            tms_tile.y
        );
        let bbox = tile.bounds(TILE_SIZE as u16);
        let bbox = Aabb2d {
            max: Vec2::new(bbox.max_x as f32, bbox.max_y as f32),
            min: Vec2::new(bbox.min_x as f32, bbox.min_y as f32),
        };

        let image: Handle<Image> = asset_server.load(url.clone());
        commands.spawn((
            Sprite { image, ..default() },
            Transform::from_translation(bbox.center().extend(1.0) * (TILE_SIZE + 1.)) // TODO: + 1 to have visible gap to differentiate tiles for testing
                .with_scale((bbox.half_size() * 2.).extend(1.0)),
            Tile,
        ));

        // TODO: no clue why these appear at the lower edge of the tile...
        commands.spawn((
            Sprite {
                custom_size: Some(Vec2::new(20., 20.)),
                color: Color::linear_rgb(1.0, 0.0, 0.0),
                ..Default::default()
            },
            Transform::from_translation(bbox.center().extend(1.0) * TILE_SIZE)
                .with_scale((bbox.half_size() * 2.).extend(1.0)),
        ));
    }
}

fn handle_zoom_level(scale: On<NewScale>) {
    dbg!(scale.event().log2());
    // TODO: convert to zoom level somehow. Seems plausible 1 scale == 1 zoom level, but the tiles loaded with zoom leve 9
    // look good at scale ~17 on my screen.
}

fn move_cam_once(
    mut cam: Single<(&mut Transform, &mut SmoothZoom), (With<Camera>, Without<Tile>)>,
    tile: Query<(&Transform), (With<Tile>, Without<Camera>)>,
) {
    let tile = tile.iter().next().unwrap();
    cam.0.translation = tile.translation;
    cam.1.target_zoom = tile.scale.x.max(tile.scale.y); // "good enough" approximation to see something useful
}

pub fn debug_draw(
    mut commands: Commands,
    camera_query: Query<(Entity, &Camera, &GlobalTransform)>,
    primary_window: Query<Entity, With<PrimaryWindow>>,
    pointers: Query<(Entity, &PointerLocation)>,
    scale: Res<UiScale>,
) {
    for (entity, location) in &pointers {
        let Some(pointer_location) = &location.location() else {
            continue;
        };
        for (cam_e, camera, cam_global_transform) in camera_query.iter().filter(|(_, camera, _)| {
            camera
                .target
                .normalize(primary_window.single().ok())
                .is_some_and(|target| target == pointer_location.target)
        }) {
            let mut pointer_pos = pointer_location.position;
            if let Some(viewport) = camera_query
                .get(cam_e)
                .ok()
                .and_then(|(_, camera, _)| camera.logical_viewport_rect())
            {
                pointer_pos -= viewport.min;
            }

            let pos = camera
                .viewport_to_world_2d(cam_global_transform, pointer_pos)
                .unwrap()
                / TILE_SIZE;
            let coords = get_projection(3857)
                .unwrap()
                .projected_to_deg(pos.x as f64, pos.y as f64);
            let text = format!("Lat: {}, Lon: {}", coords.1, coords.0);

            commands
                .entity(entity)
                .despawn_related::<Children>()
                .insert((
                    Node {
                        position_type: PositionType::Absolute,
                        left: Val::Px(pointer_pos.x + 5.0) / scale.0,
                        top: Val::Px(pointer_pos.y + 5.0) / scale.0,
                        padding: UiRect::px(10.0, 10.0, 8.0, 6.0),
                        ..Default::default()
                    },
                    BackgroundColor(Color::BLACK.with_alpha(0.75)),
                    GlobalZIndex(i32::MAX),
                    Pickable::IGNORE,
                    UiTargetCamera(cam_e),
                    children![(Text::new(text.clone()), TextFont::from_font_size(12.0))],
                ));
        }
    }
}
