use bevy::{
    camera::primitives::Aabb,
    math::bounding::{Aabb2d, BoundingVolume},
    prelude::*,
};
use tilemath::{BBox, bbox_covered_tiles};

use crate::pancam::{NewScale, SmoothZoom};

mod pancam;
use pancam::pancam_plugin;

const TILE_SIZE: f32 = 256.;

pub struct MapPlugin;

impl Plugin for MapPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(pancam_plugin)
            .add_systems(Startup, setup)
            .add_systems(PostStartup, move_cam_once)
            .add_observer(handle_zoom_level);
    }
}

#[derive(Component, Copy, Clone)]
struct Tile;

fn setup(mut commands: Commands, asset_server: Res<AssetServer>) {
    let proj = proj::Proj::new_known_crs("EPSG:4326", "EPSG:3857", None).unwrap();
    let (max_x, max_y) = proj
        .convert((14.261350595825427, 54.963283243087496))
        .unwrap();
    let (min_x, min_y) = proj
        .convert((5.5437553459687186, 46.2960593038139))
        .unwrap();
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
    dbg!(scale.event());
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

// fn lat_lon_to_bevy(latlon: Vec2) -> Vec2 {
//     let proj = proj::Proj::new_known_crs("EPSG:4326", "EPSG:3857", None).unwrap();
//     let (x, y) = proj.convert((latlon.x, latlon.y)).unwrap();
// }
