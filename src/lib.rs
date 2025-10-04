use bevy::{
    math::bounding::{Aabb2d, BoundingVolume},
    picking::pointer::PointerLocation,
    prelude::*,
    window::PrimaryWindow,
};

use tilemath::bbox_covered_tiles;

mod pancam;
use pancam::{NewScale, SmoothZoom, pancam_plugin};
mod coord_conversions;
pub use coord_conversions::{Convert, ToBBox, WebMercatorConversion};

pub const TILE_SIZE: f32 = 256.;

pub struct MapPlugin;

impl Plugin for MapPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(pancam_plugin)
            .add_systems(Startup, setup)
            .add_systems(PostStartup, move_cam_once)
            .add_systems(Update, debug_draw)
            .add_observer(handle_zoom_level);
    }
}

#[derive(Component, Copy, Clone)]
struct Tile;

fn setup(mut commands: Commands, asset_server: Res<AssetServer>) {
    let germany = Aabb2d {
        max: Vec2::new(14.261350595825427, 54.963283243087496),
        min: Vec2::new(5.5437553459687186, 46.2960593038139),
    };
    let zoom = 10;
    let tiles = bbox_covered_tiles(&germany.lonlat_to_bbox(), zoom).collect::<Vec<_>>();
    info!("Tiles to load: {:?}", tiles.len());
    for tile in tiles {
        info!("Loading tile: {:?}", tile);
        let tms_tile = tile.to_reversed_y();
        let url = format!(
            "https://mapproxy.dmho.de/tms/1.0.0/thunderforest_transport/EPSG3857/{}/{}/{}.png",
            tms_tile.zoom - 1,
            tms_tile.x,
            tms_tile.y
        );
        let bbox = tile.bounds(1);
        let bbox = Aabb2d {
            max: Vec2::new(bbox.max_x as f32, bbox.max_y as f32),
            min: Vec2::new(bbox.min_x as f32, bbox.min_y as f32),
        };

        let image: Handle<Image> = asset_server.load(url.clone());
        commands.spawn((
            Sprite {
                image,
                custom_size: Some(Vec2::ONE),
                ..default()
            },
            Transform::from_translation(bbox.center().extend(1.0))
                .with_scale((bbox.half_size() * 2.).extend(1.0)),
            Tile,
        ));

        commands.spawn((
            Sprite {
                custom_size: Some(Vec2::splat(0.1)),
                color: Color::linear_rgb(1.0, 0.0, 0.0),
                ..Default::default()
            },
            Transform::from_translation(bbox.center().extend(1.0))
                .with_scale((bbox.half_size() * 2.).extend(1.0)),
        ));
    }
}

fn handle_zoom_level(scale: On<NewScale>) {
    // dbg!(scale.event().log2());
    // TODO: convert to zoom level somehow. Seems plausible 1 scale == 1 zoom level, but the tiles loaded with zoom leve 9
    // look good at scale ~17 on my screen.

    // scale.log2() > 22 => zoom level 1 (min)
    // scale.log2() =~ 21.5 => zoom level 4
    // scale.log2() =~ 16.5 => zoom level 9
    // scale.log2() =~ 15.5 => zoom level 10
    // scale.log2() < 7 => zoom level 19 (max)
    // something like zoom_level = ((26.0 - scale.log2()*1.1).round() as u8).clamp(1, 19);
}

fn move_cam_once(
    mut cam: Single<(&mut Transform, &mut SmoothZoom), (With<Camera>, Without<Tile>)>,
    tile: Query<(&Transform), (With<Tile>, Without<Camera>)>,
) {
    let tile = tile.iter().next().unwrap();
    cam.0.translation = tile.translation;
    cam.1.target_zoom = tile.scale.x.max(tile.scale.y) / TILE_SIZE; // "good enough" approximation to see something useful
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

            let Ok(pos) = camera.viewport_to_world_2d(cam_global_transform, pointer_pos) else {
                continue;
            };

            let coords = pos.world_to_lonlat();
            let text = format!("Lat: {}, Lon: {}", coords.y, coords.x);

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
