use std::ops::RangeInclusive;

use bevy::{
    ecs::system::SystemParam, math::bounding::BoundingVolume, picking::pointer::PointerLocation,
    platform::collections::HashSet, prelude::*, sprite::Text2dShadow, window::PrimaryWindow,
};

use crate::{
    coord_conversions::tile_to_mercator_aabb,
    pancam::{MainCam, NewScale, SmoothZoom, pancam_plugin},
};
use tilemath::{Tile as TileMathTile, TileIterator};

mod coord_conversions;
mod local_origin;
mod pancam;
pub use coord_conversions::{ToBBox, ToTileCoords, ViewportConv, WebMercatorConversion};
pub use local_origin::{LocalOrigin, LocalSpace, MercatorAabb2d, MercatorCoords, TileBounds};

pub const TILE_SIZE: f32 = 256.;
pub const ZOOM_RANGE: RangeInclusive<u8> = 1..=20;

// How many tiles to keep loaded
const KEEP_UNUSED_TILES: usize = 1000;
// increase this to make zoom levels "further away" for cleanup logic - closer tiles will be cleaned later
const ZOOM_DISTANCE_FACTOR: u32 = 10;

pub const MIN_ORTHO_SCALE: f32 = 0.1;

fn zoom_to_scale(zoom: u8) -> f32 {
    let clamped = zoom.clamp(*ZOOM_RANGE.start(), *ZOOM_RANGE.end()) as i32;
    2.0f32.powf(24.5 - clamped as f32)
}

fn scale_to_zoom(scale: f32) -> u8 {
    let zoom = (24.5 - scale.log2()).round() as i32;
    zoom.clamp(*ZOOM_RANGE.start() as i32, *ZOOM_RANGE.end() as i32) as u8
}
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
        let zoom = self
            .initial_zoom
            .clamp(*ZOOM_RANGE.start(), *ZOOM_RANGE.end());
        let target_scale = zoom_to_scale(zoom);
        let initial_mercator = self
            .initial_center
            .as_dvec2()
            .lonlat_to_mercator()
            .extend(1.0);
        let origin = LocalOrigin::new(initial_mercator);
        let camera_translation = origin.mercator_to_local_vec3(initial_mercator);

        app.insert_resource(origin)
            .add_plugins(pancam_plugin)
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
                (sync_added_mercator_coords, sync_changed_mercator_coords),
            )
            .init_resource::<ExistingTilesSet>()
            .add_observer(handle_zoom_level)
            .add_observer(tile_url_to_sprite)
            .add_observer(tile_inserted)
            .add_observer(tile_replaced)
            .add_observer(keep_display_size);
    }
}

/// The zoom level of the map view
#[derive(Component)]
#[relationship(relationship_target = ZoomLevels)]
pub struct ZoomOf(Entity);

#[derive(Component)]
#[relationship_target(relationship = ZoomOf, linked_spawn)] // linked_spawn == despawn related
pub struct ZoomLevels(Vec<Entity>);

#[derive(Component, Eq, PartialEq)]
pub struct Zoom(u8);

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
struct Tile(TileMathTile);

fn handle_zoom_level(
    scale: On<NewScale>,
    cam: Single<(&mut Zoom, &ZoomLevels), Without<ZoomOf>>,
    mut zooms: Query<(&Zoom, &mut Transform, &mut Visibility), (With<ZoomOf>, Without<ZoomLevels>)>,
) {
    let (mut zoom, levels) = cam.into_inner();
    // https://www.desmos.com/calculator/dkbfdjvcfx
    let current_scale: f32 = scale.event().0;
    zoom.0 = scale_to_zoom(current_scale);
    for e in levels.iter() {
        let (level, mut tr, mut vis) = zooms.get_mut(e).unwrap();
        if level.0 == zoom.0 {
            *vis = Visibility::Inherited;
            tr.translation.z = -0.1;
        } else if level.0 == zoom.0.saturating_sub(1) {
            *vis = Visibility::Inherited;
            tr.translation.z = -0.2;
        } else if level.0 == zoom.0.saturating_add(1) {
            *vis = Visibility::Inherited;
            tr.translation.z = -0.5;
        } else {
            *vis = Visibility::Hidden;
            tr.translation.z = -1.0;
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

fn new_tile(tile: TileMathTile, origin: &LocalOrigin) -> impl Bundle {
    //let tile_coord_limit = (2 as u32).pow(tile.zoom as u32) - 1;

    let url = format!(
        "https://mapproxy.dmho.de/tms/1.0.0/thunderforest_transport/EPSG3857/{}/{}/{}.png",
        tile.zoom.saturating_sub(1),
        tile.x, // % (tile_coord_limit + 1),
        tile.y, //.clamp(0, tile_coord_limit)
    );
    let mercator_bounds = tile_to_mercator_aabb(tile);
    let mercator_center = mercator_bounds.center().extend(1.0);
    let local_bounds = origin.mercator_aabb_to_local(mercator_bounds);
    let translation = local_bounds.center().extend(1.0);
    let scale = (local_bounds.half_size() * 2.0).extend(1.0);

    (
        TileUrl(url),
        LocalSpace,
        MercatorCoords::from_vec(mercator_center),
        Transform::from_translation(translation).with_scale(scale),
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
        transform.translation = origin.mercator_to_local_vec3(coords.as_dvec3());
        commands.entity(entity).insert(LocalSpace);
    }

    for (entity, coords) in added_without_transform.iter() {
        dbg!(entity, coords);
        let translation = origin.mercator_to_local_vec3(coords.as_dvec3());
        commands
            .entity(entity)
            .insert((LocalSpace, Transform::from_translation(translation)));
    }
}

fn sync_changed_mercator_coords(
    origin: Res<LocalOrigin>,
    mut query: Query<(&MercatorCoords, &mut Transform), Changed<MercatorCoords>>,
) {
    for (coords, mut transform) in query.iter_mut() {
        transform.translation = origin.mercator_to_local_vec3(coords.as_dvec3());
    }
}

fn update_local_origin(
    mut origin: ResMut<LocalOrigin>,
    mut cam_query: Query<&mut Transform, With<MainCam>>,
    mut locals_without_coords: Query<
        &mut Transform,
        (
            With<LocalSpace>,
            Without<MainCam>,
            Without<Zoom>,
            Without<MercatorCoords>,
        ),
    >,
    mut locals_with_coords: Query<
        (&MercatorCoords, &mut Transform),
        (With<LocalSpace>, Without<MainCam>, Without<Zoom>),
    >,
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

    for (coords, mut transform) in locals_with_coords.iter_mut() {
        transform.translation = origin.mercator_to_local_vec3(coords.as_dvec3());
    }

    for mut transform in locals_without_coords.iter_mut() {
        transform.translation -= delta;
    }
}

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
            let mercator_pos = origin.local_to_mercator_vec2(pos);
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
