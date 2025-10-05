use std::cmp::Reverse;

use bevy::{
    camera::visibility::VisibleEntities,
    math::bounding::{Aabb2d, BoundingVolume, IntersectsVolume},
    picking::pointer::PointerLocation,
    platform::collections::{HashMap, HashSet},
    prelude::*,
    sprite::Text2dShadow,
    window::PrimaryWindow,
};

use crate::{
    coord_conversions::tile_to_aabb,
    pancam::{MainCam, NewScale, SmoothZoom, pancam_plugin},
};
use tilemath::{BBox, Tile as TileMathTile, TileIterator, WEB_MERCATOR_EXTENT, bbox_covered_tiles};

mod coord_conversions;
mod pancam;
pub use coord_conversions::{ToBBox, ToTileCoords, ViewportConv, WebMercatorConversion};

pub const TILE_SIZE: f32 = 256.;

pub struct MapPlugin {
    /// Initial zoom level of the map, between 1 and 19
    pub initial_zoom: u8,
    /// Initial center of the map in lon/lat (EPSG:4326 / WGS84)
    pub initial_center: Vec2,
}

impl Default for MapPlugin {
    fn default() -> Self {
        Self {
            initial_zoom: 9,
            initial_center: Vec2::new(13.4050, 52.5200), // Berlin
        }
    }
}

impl Plugin for MapPlugin {
    fn build(&self, app: &mut App) {
        let zoom = self.initial_zoom.clamp(1, 19);
        let target_zoom = (1 << (19 - self.initial_zoom)) as f32; //TODO: use better formula
        let translation = self.initial_center.lonlat_to_world().extend(1.0);
        app.add_plugins(pancam_plugin)
            .add_systems(
                PostStartup,
                (
                    move |mut cam: Single<(&mut Transform, &mut SmoothZoom), With<MainCam>>| {
                        cam.0.translation = translation;
                        cam.1.target_zoom = target_zoom;
                    },
                    move |mut commands: Commands| {
                        commands.spawn(MapZoom(zoom));
                        for lvl in 0..=19 {
                            commands.spawn((
                                ZoomLevel(lvl),
                                Transform::IDENTITY,
                                Visibility::Visible,
                            ));
                        }
                    },
                ),
            )
            .add_systems(Update, (debug_draw, spawn_new_tiles, despawn_old_tiles))
            .init_resource::<ExistingTilesSet>()
            .register_type::<MapZoom>()
            .add_observer(handle_scale_change)
            .add_observer(tile_url_to_sprite)
            .add_observer(tile_inserted)
            .add_observer(tile_replaced)
            .add_observer(handle_zoom_level_change);
    }
}

/// The zoom level of the map view
#[derive(Component, Debug, Clone, Deref, Reflect)]
pub struct MapZoom(pub u8);

/// ZoomLevel as a component with Tiles as children for easier querying
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ZoomLevel(u8);


#[derive(Component, Debug)]
#[component(immutable)]
struct Tile(TileMathTile);

#[derive(Event, Debug, Clone)]
struct ZoomLevelChange{
    old: u8,
    new: u8,
}

fn handle_zoom_level_change(
    change: On<ZoomLevelChange>,
    mut zoom_level: Query<(&ZoomLevel, &mut Transform)>
) {
    let event = change.event();

    let (old_zoom, new_zoom) = (event.old, event.new);

    for (zoom_level, mut transform) in zoom_level.iter_mut() {
        if zoom_level.0 == old_zoom {
            transform.translation.z = -0.1; // move old zoom level back
        } else if zoom_level.0 == new_zoom {
            transform.translation.z = 0.0; // move new zoom level to front
        } else {
            transform.translation.z = -0.2; // move other zoom levels far back
        }
    }
}

fn handle_scale_change(
    scale: On<NewScale>,
    mut zoom: Single<&mut MapZoom>,
    mut commands: Commands
) {
    let zoom_level = (-1.031*scale.event().log2() + 17.5).round().clamp(1.0, 19.0) as u8;

    let old_zoom = zoom.0.clone();
    if zoom_level != zoom.0 {
        zoom.0 = zoom_level;
        commands.trigger(ZoomLevelChange{old: old_zoom, new: zoom_level});
    }
    /* New:
        -1.7 19
        -0.6 18
        0.5 17
        1.5 16
        2.4 15
        3.4 14
        4.4 13
        5.6 12
        6.3 11
        7.3 10
        8.3 9
        9.4 8
        10.2 7
        11.3 6
        12.2 5
        13.3 4
        14 3
        15 2
        15.5 1
        Linear regression gives:
        zoom_level = -1.031*scale.log2() + 17.5
     */
}

#[derive(Component, Debug)]
#[component(immutable)]
struct TileUrl(String);

// Insert a Sprite for each TileUrl
fn tile_url_to_sprite(
    insert: On<Insert, TileUrl>,
    query: Query<&TileUrl>,
    asset_server: Res<AssetServer>,
    mut commands: Commands,
) {
    let Ok(tileurl) = query.get(insert.entity) else {
        return;
    };

    let image: Handle<Image> = asset_server.load(tileurl.0.clone());

    commands.entity(insert.entity).insert(Sprite {
        image,
        custom_size: Some(Vec2::ONE),
        ..default()
    });
}

fn new_tile(tile: TileMathTile) -> impl Bundle {
    //let tile_coord_limit = (2 as u32).pow(tile.zoom as u32) - 1;

    let url = format!(
        "https://mapproxy.dmho.de/tms/1.0.0/thunderforest_transport/EPSG3857/{}/{}/{}.png",
        tile.zoom - 1,
        tile.x, // % (tile_coord_limit + 1),
        tile.y, //.clamp(0, tile_coord_limit)
    );
    let bbox = tile_to_aabb(tile);

    (
        TileUrl(url),
        Transform::from_translation(bbox.center().extend(1.0))
            .with_scale((bbox.half_size() * 2.).extend(1.0)),
        Tile(tile),
        // children![(
        //     Text2d::new(format!("{}/{}/{}", tile.zoom, tile.x, tile.y)),
        //     Text2dShadow {
        //         offset: Vec2::new(2.0, -2.0),
        //         ..Default::default()
        //     },
        //     TextFont::from_font_size(20.0),
        //     Transform::from_scale(Vec3::ONE / 100.).with_translation(Vec3::Z),
        // )],
    )
}

#[derive(Resource, Debug, Default)]
struct ExistingTilesSet(HashSet<TileMathTile>);

// use component lifecycle events to keep the ExistingTilesSet up to date
// https://docs.rs/bevy/latest/bevy/ecs/lifecycle/index.html
fn tile_inserted(
    insert: On<Insert, Tile>,
    query: Query<&Tile>,
    mut existing: ResMut<ExistingTilesSet>,
) {
    let tile = query.get(insert.entity).unwrap();
    existing.0.insert(tile.0);
}

fn tile_replaced(
    replace: On<Replace, Tile>,
    query: Query<&Tile>,
    mut existing: ResMut<ExistingTilesSet>,
) {
    let tile = query.get(replace.entity).unwrap();
    existing.0.remove(&tile.0);
}

fn spawn_new_tiles(
    mut commands: Commands,
    zoom: Single<&MapZoom>,
    view: ViewportConv<MainCam>,
    existing_tiles: Res<ExistingTilesSet>,
    zoom_level: Query<(Entity, &ZoomLevel)>
) -> Result<()> {
    let zoom = zoom.0;
    let bbox = view.visible_aabb()?.world_to_tile_coords(zoom);
    let current_view_tiles = TileIterator::new(
        zoom,
        (bbox.min.x as u32)..=(bbox.max.x as u32),
        (bbox.min.y as u32)..=(bbox.max.y as u32),
    )
    .collect::<HashSet<_>>();
    let diff = current_view_tiles.difference(&existing_tiles.0);
    let Some((zoom_entity, _)) = zoom_level.iter().find(|(_, zl)| zl.0 == zoom) else {
        error!("No ZoomLevel entity found for zoom level {zoom}");
        return Ok(());
    };
    commands.entity(zoom_entity).with_children(|commands| {
        for tile in diff {
            commands.spawn(
                new_tile(*tile)
            );
        }
    });
    Ok(())
}

const ZOOM_DISTANCE_FACTOR: u32 = 10;

fn despawn_old_tiles(
    mut commands: Commands,
    zoom: Single<&MapZoom>,
    view: ViewportConv<MainCam>,
    tiles: Query<(Entity, &Tile, &ViewVisibility)>,
) -> Result<()> {
    let tiles = tiles.iter().filter(|(_, _, vis)| !vis.get());
    if tiles.clone().count() < 1000 {
        return Ok(());
    }
    let mut tiles = tiles.collect::<Vec<_>>();
    let zoom = zoom.0;
    let center = view
        .viewport_center_world()
        .unwrap()
        .world_to_tile_coords(zoom);
    let me = center.extend(zoom as u32 * ZOOM_DISTANCE_FACTOR);
    // manhattan distance is cheap and good enough. maybe even better for this than euclidian
    tiles.sort_unstable_by_key(|(_, a, _)| {
        me.manhattan_distance(UVec3::new(
            a.0.x,
            a.0.y,
            a.0.zoom as u32 * ZOOM_DISTANCE_FACTOR,
        ))
    });
    let mut count = 0;
    for (e, _, _) in tiles.iter().skip(1000) {
        commands.entity(*e).despawn();
        count += 1;
    }
    Ok(())
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
            let text = format!(
                "Lat: {}, Lon: {}, web x: {}, web y: {}",
                coords.y, coords.x, pos.x, pos.y
            );

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