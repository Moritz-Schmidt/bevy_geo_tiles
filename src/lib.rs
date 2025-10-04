use bevy::{
    log::tracing_subscriber::field::debug, math::bounding::{Aabb2d, BoundingVolume, IntersectsVolume}, picking::pointer::PointerLocation, platform::collections::HashSet, prelude::*, sprite::Text2dShadow, window::PrimaryWindow
};

use crate::{coord_conversions::ToTileCoords, pancam::{pancam_plugin, NewScale, SmoothZoom}};
use tilemath::{bbox_covered_tiles, BBox, Tile as TileMathTile, TileIterator, WEB_MERCATOR_EXTENT};

mod pancam;
mod coord_conversions;
pub use coord_conversions::{Convert, ToBBox, WebMercatorConversion};

pub const TILE_SIZE: f32 = 256.;

pub struct MapPlugin {
    /// Initial zoom level of the map, between 1 and 19
    pub initial_zoom: u8,
    /// Initial center of the map in lon/lat (EPSG:4326 / WGS84)
    pub initial_center: Vec2
}

impl Default for MapPlugin {
    fn default() -> Self {
        Self {
            initial_zoom: 9,
            initial_center: Vec2::new(13.4050, 52.5200), // Berlin
        }
    }
}

#[derive(Resource, Debug, Clone)]
pub struct MapConfig {
    zoom: u8,
    center: Vec2
}

impl Plugin for MapPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(pancam_plugin)
            .insert_resource(MapConfig {
                zoom: self.initial_zoom.clamp(1, 19),
                center: self.initial_center,
            })
            .add_systems(PostStartup, (setup_camera, setup_map_view).chain())
            .add_systems(Update, (debug_draw, viewport_tiles))
            .add_observer(map_view_changed)
            .add_observer(handle_zoom_level);
    }
}

fn setup_camera(
    map_config: Res<MapConfig>,
    mut cam: Single<(&mut Transform, &mut SmoothZoom), (With<Camera>,Without<Tile>)>,
) {
    debug!("Map config: {:?}", map_config);
    let (mut cam_transform, mut smooth_zoom) = cam.into_inner();
    let cam_world = map_config.center.lonlat_to_world();
    
    cam_transform.translation = cam_world.extend(1.0);
    let scale = (1 << (19 - map_config.zoom)) as f32; //TODO: use better formula
    smooth_zoom.target_zoom = scale;
}

fn setup_map_view(
    map_config: Res<MapConfig>,
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
        (tr, bl)
    } else {
        return;
    };
    let bbox = Aabb2d {
        min: bottom_left,
        max: top_right,
    };
    let tile_bbox = bbox.world_to_tile_coords(map_config.zoom);
    commands.spawn(MapView {
        zoom: map_config.zoom,
        bbox,
        tile_bbox,
    });
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
        let tile_bbox = tile.0.bounds(1);
        let tile_aabb = Aabb2d {
            min: Vec2::new(tile_bbox.min_x as f32, tile_bbox.min_y as f32),
            max: Vec2::new(tile_bbox.max_x as f32, tile_bbox.max_y as f32),
        };
        self.bbox.intersects(&tile_aabb)
    }
}

#[derive(Event, Debug)]
struct MapViewChanged;

fn handle_zoom_level(scale: On<NewScale>) {
    dbg!(scale.event().log2());
    // dbg!(scale.event().log2());
    // scale.log2() =~ 21.5 => zoom level 4
    // scale.log2() =~ 16.5 => zoom level 9
    // scale.log2() =~ 15.5 => zoom level 10
    // scale.log2() < 7 => zoom level 19 (max)
    // something like zoom_level = ((26.0 - scale.log2()*1.1).round() as u8).clamp(1, 19);
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
        (tr, bl)
    } else {
        return;
    };
    let bbox = Aabb2d {
        min: bottom_left,
        max: top_right,
    };
    if map_view.bbox != bbox {
        let tile_bbox = bbox.world_to_tile_coords(map_view.zoom);
        map_view.bbox = bbox;
        if map_view.tile_bbox != tile_bbox {
            map_view.tile_bbox = tile_bbox;
            commands.trigger(MapViewChanged);
        }
    }
}

fn map_view_changed(
    _event: On<MapViewChanged>,
    mut commands: Commands,
    map_view: Single<&MapView>,
    tiles_rendered: Query<(Entity, Option<&Tile>)>,
    asset_server: Res<AssetServer>,
) {
    let map_view = map_view.into_inner();
    debug!("Map view changed: {:?}", map_view);

    let tiles_in_view = TileIterator::new(
        map_view.zoom,
        (map_view.tile_bbox.min.x as u32)..=(map_view.tile_bbox.max.x as u32),
        (map_view.tile_bbox.min.y as u32)..=(map_view.tile_bbox.max.y as u32),
    ).collect::<HashSet<_>>();
    
    
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
    let tile_coord_limit = (2 as u32).pow(map_view.zoom as u32) - 1;

    for tile in tiles_to_render {
        info!("Loading tile: {:?}", tile);
        let tms_tile = tile.to_reversed_y();
        let url = format!(
            "https://mapproxy.dmho.de/tms/1.0.0/thunderforest_transport/EPSG3857/{}/{}/{}.png",
            tms_tile.zoom - 1,
            tms_tile.x % (tile_coord_limit + 1),
            tms_tile.y.clamp(0, tile_coord_limit)
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
            Tile(tile),
        ));


        commands.spawn((
            Text2d::new(format!("{}/{}/{}", tile.zoom, tile.x, tile.y)),
            Text2dShadow {offset: Vec2::new(4.0, -4.0), ..Default::default()},
            TextFont::from_font_size(200.0),
            Transform::from_translation(bbox.center().extend(1.1))
                .with_scale((bbox.half_size() * 0.001).extend(1.0)),
            Tile(tile),
        ));
    }
}
