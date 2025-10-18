//! Render slippy map tiles inside a Bevy application without worrying about floating point drift
//! or tile fetching boilerplate.
//!
//! # Features
//! * Web Mercator (`EPSG:3857`) coordinate helpers built on `f64` precision.
//! * A moving local origin that keeps Bevy world coordinates in a range that avoids floating point precision issues.
//! * Automatic spawning, streaming, and culling of tiles for the active zoom level.
//! * Pluggable tile downloader with configurable headers, URL templates, reverse-Y support, and on-disk caching.
//! * Convenience components – for example [`MercatorCoords`] – so users can place objects on the map in projected coordinates.
//!
//! # Quick start
//! Add the crate to `Cargo.toml` and register the [`MapPlugin`] alongside Bevy’s default plugins:
//!
//! ```no_run
//! use bevy::prelude::*;
//! use bevy_geo_tiles::MapPlugin;
//!
//! fn main() {
//!     App::new()
//!         .add_plugins(DefaultPlugins)
//!         .add_plugins(MapPlugin::default())
//!         .run();
//! }
//! ```
//!
//! To place an entity at a specific latitude/longitude pair, add the [`MercatorCoords`] component – the
//! plugin keeps its transform in sync with the current local origin:
//!
//! ```no_run
//! use bevy::prelude::*;
//! use bevy_geo_tiles::{KeepDisplaySize, MapPlugin, MercatorCoords};
//!
//! fn main() {
//!     App::new()
//!         .add_plugins(DefaultPlugins)
//!         .add_plugins(MapPlugin::default())
//!         .add_systems(Startup, spawn_marker)
//!         .run();
//! }
//!
//! fn spawn_marker(mut commands: Commands) {
//!     commands.spawn((
//!         Sprite {
//!             color: Color::linear_rgb(1.0, 0.0, 0.0),
//!             custom_size: Some(Vec2::splat(1.0)),
//!             ..Default::default()
//!         },
//!         MercatorCoords::from_latlon(52.5200, 13.4050).with_z(5.0),
//!         KeepDisplaySize, // optional: keeps the sprite size constant on screen when zooming
//!     ));
//! }
//! ```
//!
//! # Coordinate systems
//! * **Mercator space** uses `DVec2`/`DVec3` in meters relative to the Web Mercator map projection.
//! * **Local space** is Bevy’s world coordinate system (floating point `Vec2`/`Vec3`). The [`LocalOrigin`] resource
//!   tracks the current offset between the two and recenters automatically when the camera drifts too far from the origin.
//!
//! See [`MapPlugin`] for configuration options, including tile server customization and cache settings.
#![allow(clippy::type_complexity)]

use std::{ops::RangeInclusive, path::PathBuf};

use bevy::{
    ecs::system::SystemParam, math::bounding::BoundingVolume, picking::pointer::PointerLocation,
    platform::collections::HashSet, prelude::*, window::PrimaryWindow,
};

use crate::{
    coord_conversions::tile_to_mercator_aabb,
    pancam::{MainCam, NewScale, SmoothZoom, pancam_plugin},
    shapes::shapes_plugin,
    tile_fetcher::{
        TileFetcher, apply_tile_fetch_results, default_cache_dir, queue_tile_downloads,
    },
};
use tilemath::{Tile as TileMathTile, TileIterator};

mod coord_conversions;
mod local_origin;
mod local_origin_conversions;
mod pancam;
pub mod shapes;
mod tile_fetcher;
pub use coord_conversions::{ToBBox, ToTileCoords, ViewportConv, WebMercatorConversion};
pub use local_origin::{LocalOrigin, LocalSpace, MercatorAabb2d, MercatorCoords};
pub use local_origin_conversions::LocalOriginConversion;
pub use tile_fetcher::{TileFetchConfig, TileTextureError};

pub const TILE_SIZE: f32 = 256.;
pub const ZOOM_RANGE: RangeInclusive<u8> = 1..=18;

// How many tiles to keep loaded
const KEEP_UNUSED_TILES: usize = 1000;
// increase this to make zoom levels "further away" for cleanup logic - closer tiles will be cleaned later
const ZOOM_DISTANCE_FACTOR: u32 = 10;

pub const MIN_ORTHO_SCALE: f32 = 0.1;

fn zoom_to_scale(zoom: u8, zoom_offset: i8) -> f32 {
    let clamped =
        zoom.clamp(*ZOOM_RANGE.start(), *ZOOM_RANGE.end()) as i32 - 1 - zoom_offset as i32;
    2.0f32.powf(24.5 - clamped as f32)
}

fn scale_to_zoom(scale: f32, zoom_offset: i8) -> u8 {
    let zoom = (24.5 - scale.log2()).round() as i32 - 1 - zoom_offset as i32;
    zoom.clamp(*ZOOM_RANGE.start() as i32, *ZOOM_RANGE.end() as i32) as u8
}

/// Bevy plugin for displaying slippy map tiles from a tile server (e.g. OpenStreetMap).
///
/// This plugin handles the fetching and displaying of map tiles, as well as managing the camera.
/// It also exposes some components for working with the map.
pub struct MapPlugin {
    /// Initial zoom level of the map, between 1 and 19
    pub initial_zoom: u8,
    /// Initial center of the map in lon/lat (EPSG:4326 / WGS84)
    pub initial_center: Vec2,
    /// Whether to use TMS-style Y coordinates (origin bottom-left) instead of XYZ-style (origin top-left).
    pub reverse_y: bool,
    /// zoom level offset applied when fetching tiles (can be negative).
    /// For example, with an offset of -1, tile 3/4/2 will be fetched when tile 4/4/2 is requested.
    pub zoom_offset: i8,
    /// Tile source URL template, e.g. "https://tile.openstreetmap.org/{z}/{x}/{y}.png"
    pub tile_source: String,
    /// headers to add to tile requests
    /// Defaults to: `User-Agent: bevy-geo-tiles/0.1`
    pub headers: Vec<(String, String)>,
    /// Directory to use for caching tiles locally
    /// Defaults to: `std::env::temp_dir()/bevy-geo-tiles-cache`
    pub cache_directory: PathBuf,
}

impl Default for MapPlugin {
    fn default() -> Self {
        Self {
            initial_zoom: 9,
            initial_center: Vec2::new(13.4050, 52.5200), // Berlin
            reverse_y: false,
            zoom_offset: 0,
            tile_source: "https://tile.openstreetmap.org/{z}/{x}/{y}.png".to_string(),
            headers: vec![("User-Agent".to_string(), "bevy-geo-tiles/0.1".to_string())],
            cache_directory: default_cache_dir(),
        }
    }
}

impl Plugin for MapPlugin {
    fn build(&self, app: &mut App) {
        let zoom = self
            .initial_zoom
            .clamp(*ZOOM_RANGE.start(), *ZOOM_RANGE.end());
        let target_scale = zoom_to_scale(zoom, self.zoom_offset);
        let initial_mercator = self
            .initial_center
            .as_dvec2()
            .lonlat_to_mercator()
            .extend(1.0);
        let origin = LocalOrigin::new(initial_mercator);
        let camera_translation = initial_mercator.mercator_to_local(&origin).as_vec3();

        app.insert_resource(TileFetchConfig {
            url_template: self.tile_source.clone(),
            headers: self.headers.iter().cloned().collect(),
            cache_directory: self.cache_directory.clone(),
            reverse_y: self.reverse_y,
            zoom_offset: self.zoom_offset,
            cache_extension: "png".to_string(),
        })
        .init_resource::<TileFetcher>()
        .insert_resource(origin)
        .add_plugins(pancam_plugin)
        .add_plugins(shapes_plugin)
        .add_systems(
            Startup,
            (move |mut commands: Commands| {
                commands
                    .spawn((
                        Camera2d,
                        SmoothZoom { target_scale },
                        MainCam,
                        LocalSpace,
                        Transform::from_translation(camera_translation)
                            .with_scale(Vec3::splat(0.01)),
                        Zoom(zoom),
                    ))
                    .with_related_entities::<ZoomOf>(|rel_c| {
                        for z in ZOOM_RANGE {
                            rel_c.spawn((
                                Zoom(z),
                                Transform::default(),
                                Visibility::Inherited,
                                LocalSpace,
                            ));
                        }
                    });
            },),
        )
        .add_systems(
            Update,
            (
                update_local_origin,
                debug_draw,
                spawn_new_tiles,
                despawn_old_tiles,
            ),
        )
        .add_systems(
            PostUpdate,
            (
                sync_added_mercator_coords,
                sync_changed_mercator_coords,
                queue_tile_downloads,
                apply_tile_fetch_results,
            ),
        )
        .init_resource::<ExistingTilesSet>()
        .add_observer(handle_zoom_level)
        .add_observer(tile_inserted)
        .add_observer(tile_replaced)
        .add_observer(keep_display_size)
        .add_observer(update_locals_with_coords_on_origin_change);
    }
}

/// The zoom level of the map view
#[derive(Component)]
#[relationship(relationship_target = ZoomLevels)]
pub(crate) struct ZoomOf(Entity);

#[derive(Component)]
#[relationship_target(relationship = ZoomOf, linked_spawn)] // linked_spawn == despawn related
pub(crate) struct ZoomLevels(Vec<Entity>);

#[derive(Component, Eq, PartialEq)]
pub(crate) struct Zoom(u8);

#[derive(SystemParam)]
struct ZoomHelper<'w, 's, M: Component> {
    cam: Single<'w, 's, (&'static Zoom, &'static ZoomLevels), With<M>>,
}

impl<'w, 's, M: Component> ZoomHelper<'w, 's, M> {
    fn level_entity(&self) -> Entity {
        let index = (self.cam.0.0.saturating_sub(*ZOOM_RANGE.start())) as usize;
        self.cam.1.iter().nth(index).unwrap()
    }
    fn level(&self) -> u8 {
        self.cam.0.0
    }
}

#[derive(Component, Debug)]
#[component(immutable)]
pub struct Tile(pub TileMathTile);

fn handle_zoom_level(
    scale: On<NewScale>,
    cam: Single<(&mut Zoom, &ZoomLevels), Without<ZoomOf>>,
    mut zooms: Query<(&Zoom, &mut Transform, &mut Visibility), (With<ZoomOf>, Without<ZoomLevels>)>,
    tile_fetch_config: Res<TileFetchConfig>,
) {
    let (mut zoom, levels) = cam.into_inner();
    // https://www.desmos.com/calculator/dkbfdjvcfx
    let current_scale: f32 = scale.event().0;
    zoom.0 = scale_to_zoom(current_scale, tile_fetch_config.zoom_offset);
    for e in levels.iter() {
        let (level, mut tr, mut vis) = zooms.get_mut(e).unwrap();
        if level.0 == zoom.0 {
            *vis = Visibility::Inherited;
            tr.translation.z = -1.0;
        } else if level.0 == zoom.0.saturating_sub(1) {
            *vis = Visibility::Inherited;
            tr.translation.z = -1.2;
        } else if level.0 == zoom.0.saturating_add(1) {
            *vis = Visibility::Inherited;
            tr.translation.z = -1.5;
        } else {
            *vis = Visibility::Hidden;
            tr.translation.z = -2.0;
        }
    }
    // qry: Query<(&Tile,)>, view: ViewportConv<MainCam>
    // dbg!(scale.event().log2());
    // let tile = qry.iter().next().unwrap();
    // let aabb2 = tile_to_aabb(tile.0.0);
    // let left2 = view.world_to_viewport(aabb2.max.extend(0.)).unwrap();
    // let right2 = view
    //     .world_to_viewport(Vec2::new(aabb2.min.x, aabb2.max.y).extend(0.))
    //     .unwrap();
    // let tile_edge_width_in_pixels = left2.distance(right2);
    // dbg!(tile_edge_width_in_pixels);
}

fn new_tile(tile: TileMathTile, origin: &LocalOrigin) -> impl Bundle {
    //let tile_coord_limit = (2 as u32).pow(tile.zoom as u32) - 1;

    let mercator_bounds = tile_to_mercator_aabb(tile);
    let mercator_center = mercator_bounds.center().extend(-1.0);
    let local_bounds = mercator_bounds.mercator_to_local(origin);
    let translation = local_bounds.center().extend(-1.0);
    let scale = (local_bounds.half_size() * 2.0).extend(1.0);

    (
        LocalSpace,
        MercatorCoords::from_vec(mercator_center),
        Transform::from_translation(translation).with_scale(scale),
        GlobalTransform::default(),
        Visibility::Inherited,
        InheritedVisibility::default(),
        Tile(tile),
        // children![(
        //     Text2d::new(format!("{}/{}/{}", tile.zoom, tile.x, tile.y)),
        //     Text2dShadow {
        //         offset: Vec2::new(2.0, -2.0),
        //         ..Default::default()
        //     },
        //     TextFont::from_font_size(100.0),
        //     Transform::from_scale(Vec3::ONE / 1024.).with_translation(Vec3::Z),
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

fn sync_added_mercator_coords(
    mut commands: Commands,
    origin: Res<LocalOrigin>,
    mut with_transform: Query<
        (Entity, &MercatorCoords, &mut Transform),
        (Added<MercatorCoords>, With<Transform>),
    >,
    added_without_transform: Query<
        (Entity, &MercatorCoords),
        (Added<MercatorCoords>, Without<Transform>),
    >,
) {
    for (entity, coords, mut transform) in with_transform.iter_mut() {
        transform.translation = coords.0.mercator_to_local(&origin).as_vec3();
        commands.entity(entity).insert(LocalSpace);
    }

    for (entity, coords) in added_without_transform.iter() {
        let translation = coords.0.mercator_to_local(&origin).as_vec3();
        commands.entity(entity).insert((
            LocalSpace,
            Transform::from_translation(translation),
            GlobalTransform::default(),
        ));
    }
}

fn sync_changed_mercator_coords(
    origin: Res<LocalOrigin>,
    mut query: Query<(&MercatorCoords, &mut Transform), Changed<MercatorCoords>>,
) {
    for (coords, mut transform) in query.iter_mut() {
        transform.translation = coords.0.mercator_to_local(&origin).as_vec3();
    }
}

#[derive(Event, Debug, Clone)]
struct LocalOriginUpdated(Vec3);

fn update_local_origin(
    mut commands: Commands,
    mut origin: ResMut<LocalOrigin>,
    mut cam_query: Query<&mut Transform, With<MainCam>>,
) {
    let camera_offset = cam_query
        .single()
        .expect("Main camera missing for local origin maintenance")
        .translation
        .truncate();

    if (camera_offset.length() as f64) <= origin.recenter_distance() {
        return;
    }

    let delta = Vec3::new(camera_offset.x, camera_offset.y, 0.0);
    origin.shift_mercator_origin(delta.as_dvec3());

    for mut cam in cam_query.iter_mut() {
        cam.translation -= delta;
    }

    commands.trigger(LocalOriginUpdated(delta));
}

fn update_locals_with_coords_on_origin_change(
    event: On<LocalOriginUpdated>,
    mut locals_with_coords: Query<
        &mut Transform,
        (With<LocalSpace>, Without<MainCam>, Without<Zoom>),
    >,
) {
    let delta = event.event().0;
    for mut transform in locals_with_coords.iter_mut() {
        transform.translation -= delta;
    }
}

/// Marker component to keep the display size of an entity constant when zooming in/out
///
/// Changes the scale of the transform based on the zoom level
#[derive(Component, Debug)]
pub struct KeepDisplaySize;

fn keep_display_size(
    scale: On<NewScale>,
    mut query: Query<&mut Transform, (With<MercatorCoords>, With<KeepDisplaySize>)>,
) {
    let scale = scale.event().0 * 0.1;
    for mut tr in query.iter_mut() {
        tr.scale = Vec2::splat(scale).extend(1.0);
    }
}

fn spawn_new_tiles(
    mut commands: Commands,
    zoom: ZoomHelper<MainCam>,
    view: ViewportConv<MainCam>,
    existing_tiles: Res<ExistingTilesSet>,
    origin: Res<LocalOrigin>,
) -> Result<()> {
    let bbox = view.visible_mercator_aabb()?;
    let tile_bounds = bbox.mercator_to_tile_coords(zoom.level());
    let current_view_tiles =
        TileIterator::new(zoom.level(), tile_bounds.x_range(), tile_bounds.y_range())
            .collect::<HashSet<_>>();
    let diff = current_view_tiles.difference(&existing_tiles.0);
    //dbg!(current_view_tiles.len());
    for tile in diff {
        commands
            .entity(zoom.level_entity())
            .with_child(new_tile(*tile, &origin));
    }
    Ok(())
}

fn despawn_old_tiles(
    mut commands: Commands,
    zoom: ZoomHelper<MainCam>,
    view: ViewportConv<MainCam>,
    tiles: Query<(Entity, &Tile, &ViewVisibility)>,
) -> Result<()> {
    let tiles = tiles.iter().filter(|(_, _, vis)| !vis.get());
    if tiles.clone().count() < KEEP_UNUSED_TILES {
        return Ok(());
    }
    let mut tiles = tiles.collect::<Vec<_>>();
    let center = view
        .viewport_center_mercator()
        .unwrap()
        .mercator_to_tile_coords(zoom.level());
    let me = center.extend(zoom.level() as u32 * ZOOM_DISTANCE_FACTOR);
    // manhattan distance is cheap and good enough. maybe even better for this than euclidian
    tiles.sort_unstable_by_key(|(_, a, _)| {
        me.manhattan_distance(UVec3::new(
            a.0.x,
            a.0.y,
            a.0.zoom as u32 * ZOOM_DISTANCE_FACTOR,
        ))
    });
    for (e, _, _) in tiles.iter().skip(KEEP_UNUSED_TILES) {
        commands.entity(*e).despawn();
    }
    Ok(())
}

pub fn debug_draw(
    mut commands: Commands,
    camera_query: Query<(Entity, &Camera, &GlobalTransform)>,
    primary_window: Query<Entity, With<PrimaryWindow>>,
    pointers: Query<(Entity, &PointerLocation)>,
    scale: Res<UiScale>,
    origin: Res<LocalOrigin>,
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
            let mercator_pos = pos.local_to_mercator(&origin);
            let coords = mercator_pos.mercator_to_lonlat();
            let text = format!(
                "Lat: {}, Lon: {},\n mercator x: {}, mercator y: {},\n local x: {}, local y: {}",
                coords.y, coords.x, mercator_pos.x, mercator_pos.y, pos.x, pos.y
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
