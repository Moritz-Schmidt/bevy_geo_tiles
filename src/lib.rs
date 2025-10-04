use bevy::{
    log::tracing_subscriber::field::debug, math::bounding::{Aabb2d, BoundingVolume, IntersectsVolume}, picking::pointer::{PointerId, PointerLocation}, platform::collections::HashSet, prelude::*, sprite::Text2dShadow, window::PrimaryWindow
};

use crate::pancam::{NewScale, SmoothZoom};
use miniproj::get_projection;
use tilemath::{bbox_covered_tiles, BBox, Tile as TileMathTile, WEB_MERCATOR_EXTENT};

mod pancam;
use pancam::pancam_plugin;

const TILE_SIZE: f32 = 256.;

pub struct MapPlugin;

impl Plugin for MapPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(pancam_plugin)
            .add_systems(Startup, (setup))
            .add_systems(PostStartup, (move_cam_once, spawn_map_view).chain())
            .add_systems(Update, (debug_draw, viewport_tiles))
            .add_observer(map_view_changed)
            .add_observer(handle_zoom_level);
    }
}



#[derive(Component, Debug, Clone)]
struct MapView{
    /// The zoom level of the map view
    zoom: u8,
    /// The bounding box of the map view in web mercator coordinates
    bbox: Aabb2d,
    /// The bounding box of the tiles in the map view
    tile_bbox: Aabb2d,
}

#[derive(Component, Debug)]
struct Tile(TileMathTile);

impl MapView {
    fn contains_tile(&self, tile: &Tile) -> bool {
        if tile.0.zoom != self.zoom {
            return false;
        }
        let tile_bbox = tile.0.bounds(TILE_SIZE as u16);
        let tile_aabb = Aabb2d {
            min: Vec2::new(tile_bbox.min_x as f32, tile_bbox.min_y as f32),
            max: Vec2::new(tile_bbox.max_x as f32, tile_bbox.max_y as f32),
        };
        self.bbox.intersects(&tile_aabb)
    }
}

#[derive(Event, Debug)]
struct MapViewChanged;


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
    let zoom = 8;
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
            Tile(tile),
        ));

        // TODO: no clue why these appear at the lower edge of the tile...
        commands.spawn((
            Sprite {
                custom_size: Some(Vec2::new(20., 20.)),
                color: Color::linear_rgb(1.0, 0.0, 0.0),
                ..Default::default()
            },
            Transform::from_translation(bbox.center().extend(1.1) * TILE_SIZE)
                .with_scale((bbox.half_size() * 2.).extend(1.0)),
        ));
    }
}

fn handle_zoom_level(scale: On<NewScale>) {
    //dbg!(scale.event().log2());
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
    cam.1.target_zoom = tile.scale.x.max(tile.scale.y); // "good enough" approximation to see something useful
}

fn spawn_map_view(
    camera_query: Single<(&Camera, &GlobalTransform)>,
    mut commands: Commands,
) {
    let (camera, cam_global_transform) = camera_query.into_inner();
    let Some(viewport) = camera.logical_viewport_rect() else {
        return;
    };

    let top_right = camera.viewport_to_world_2d(cam_global_transform, Vec2::new(viewport.max.x, viewport.min.y));
    let bottom_left = camera.viewport_to_world_2d(cam_global_transform, Vec2::new(viewport.min.x, viewport.max.y));
    let (top_right, bottom_left) = if let (Ok(tr), Ok(bl)) = (top_right, bottom_left) {
        (tr / TILE_SIZE, bl / TILE_SIZE)
    } else {
        return;
    };
    let bbox = Aabb2d {
        min: bottom_left,
        max: top_right,
    };
    let tile_size_meters = (WEB_MERCATOR_EXTENT as f32 * 2.0) / f32::from((1 << 9) as i16);
    let tile_bbox = Aabb2d {
            min: Vec2::new(
                ((bbox.min.x + WEB_MERCATOR_EXTENT as f32) / tile_size_meters).floor(),
                ((WEB_MERCATOR_EXTENT as f32 - bbox.min.y) / tile_size_meters).ceil() - 1.0
            ),
            max: Vec2::new(
                ((bbox.max.x + WEB_MERCATOR_EXTENT as f32) / tile_size_meters).ceil() - 1.0,
                ((WEB_MERCATOR_EXTENT as f32 - bbox.max.y) / tile_size_meters).floor()
            ),
        };
    commands.spawn((
        MapView {
            zoom: 8,
            bbox,
            tile_bbox,
        },
    ));
    debug!("Spawned initial map view: {:?}", bbox);
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

            let pos = if let Ok(pos) = camera.viewport_to_world_2d(cam_global_transform, pointer_pos) {
                pos / TILE_SIZE
            } else {
                continue;
            };

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


fn viewport_tiles(
    camera_query: Single<(&Camera, &GlobalTransform)>,
    mut map_view: Single<&mut MapView>,
    mut commands: Commands,
) {
    let (camera, cam_global_transform) = camera_query.into_inner();
    let Some(viewport) = camera.logical_viewport_rect() else {
        return;
    };

    let top_right = camera.viewport_to_world_2d(cam_global_transform, Vec2::new(viewport.max.x, viewport.min.y));
    let bottom_left = camera.viewport_to_world_2d(cam_global_transform, Vec2::new(viewport.min.x, viewport.max.y));
    let (top_right, bottom_left) = if let (Ok(tr), Ok(bl)) = (top_right, bottom_left) {
        (tr / TILE_SIZE, bl / TILE_SIZE)
    } else {
        return;
    };
    let bbox = Aabb2d {
        min: bottom_left,
        max: top_right,
    };
    if map_view.bbox != bbox {
        let tile_size_meters = (WEB_MERCATOR_EXTENT as f32 * 2.0) / f32::from((1 << map_view.zoom) as i16);
        let tile_bbox = Aabb2d {
            min: Vec2::new(
                ((bbox.min.x + WEB_MERCATOR_EXTENT as f32) / tile_size_meters).floor(),
                ((WEB_MERCATOR_EXTENT as f32 - bbox.min.y) / tile_size_meters).ceil() - 1.0
            ),
            max: Vec2::new(
                ((bbox.max.x + WEB_MERCATOR_EXTENT as f32) / tile_size_meters).ceil() - 1.0,
                ((WEB_MERCATOR_EXTENT as f32 - bbox.max.y) / tile_size_meters).floor()
            ),
        };
        map_view.bbox = bbox;
        if map_view.tile_bbox != tile_bbox {
            map_view.tile_bbox = tile_bbox;
            commands.trigger(MapViewChanged);
        }
    }
}

fn map_view_changed(
    event: On<MapViewChanged>,
    mut commands: Commands,
    map_view: Single<&MapView>,
    tiles_rendered: Query<(Entity, Option<&Tile>)>,
    asset_server: Res<AssetServer>,
) {
    let map_view = map_view.into_inner();
    debug!("Map view changed: {:?}", map_view);
    let bbox = BBox {
        min_x: map_view.bbox.min.x as f64,
        min_y: map_view.bbox.min.y as f64,
        max_x: map_view.bbox.max.x as f64,
        max_y: map_view.bbox.max.y as f64,
    };
    let tiles_in_view = bbox_covered_tiles(&bbox, map_view.zoom).collect::<HashSet<_>>();
    
    
    let tiles_to_render = tiles_in_view
        .difference(&tiles_rendered.iter().filter_map(|(_, t)| t.map(|t| t.0)).collect())
        .cloned()
        .collect::<Vec<_>>();

    for (entity, tile) in tiles_rendered {
        let Some(tile) = tile else {
            continue;
        };
        if !map_view.contains_tile(tile) {
            // not in view anymore
            commands.entity(entity).despawn();
        }
    }

    for tile in tiles_to_render {
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
            Tile(tile),
        ));

        // TODO: no clue why these appear at the lower edge of the tile...
        commands.spawn((
            Text2d::new(format!("{}/{}/{}", tile.zoom, tile.x, tile.y)),
            TextFont::from_font_size(20.0),
            Transform::from_translation(bbox.center().extend(1.1) * TILE_SIZE)
                .with_scale((bbox.half_size() * 2.).extend(1.0)),
            Tile(tile),
        ));
    }
}
