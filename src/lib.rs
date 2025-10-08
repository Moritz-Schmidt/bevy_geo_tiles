use std::{cmp::Reverse, ops::RangeInclusive};

use bevy::{
    camera::visibility::VisibleEntities,
    ecs::system::SystemParam,
    math::bounding::{Aabb2d, BoundingVolume, IntersectsVolume},
    picking::pointer::PointerLocation,
    platform::collections::{HashMap, HashSet},
    prelude::*,
    sprite::Text2dShadow,
    window::PrimaryWindow,
};

use crate::{
    coord_conversions::tile_to_aabb_world,
    pancam::{MainCam, NewScale, SmoothZoom, pancam_plugin},
};
use tilemath::{BBox, Tile as TileMathTile, TileIterator, WEB_MERCATOR_EXTENT, bbox_covered_tiles};

mod coord_conversions;
mod pancam;
pub use coord_conversions::{MAP_SCALE, ToBBox, ToTileCoords, ViewportConv, WebMercatorConversion};

pub const TILE_SIZE: f32 = 256.;
pub const ZOOM_RANGE: RangeInclusive<u8> = 0..=20;

// How many tiles to keep loaded
const KEEP_UNUSED_TILES: usize = 1000;
// increase this to make zoom levels "further away" for cleanup logic - closer tiles will be cleaned later
const ZOOM_DISTANCE_FACTOR: u32 = 10;
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
        let target_zoom = 0.5e-2; //2f32.powf(105.0 + zoom as f32 + 0.5); // formula from below, zoomed out a bit further
        let translation = self
            .initial_center
            .as_dvec2()
            .lonlat_to_world()
            .extend(1.0)
            .as_vec3();
        app.add_plugins(pancam_plugin)
            .add_systems(
                Startup,
                (move |mut commands: Commands| {
                    commands
                        .spawn((
                            Camera2d,
                            SmoothZoom::default(),
                            MainCam,
                            Transform::from_translation(translation)
                                .with_scale(Vec3::splat(target_zoom)),
                            Zoom(zoom),
                        ))
                        .with_related_entities::<ZoomOf>(|rel_c| {
                            for z in ZOOM_RANGE {
                                rel_c.spawn((Zoom(z), Transform::default(), Visibility::Inherited));
                            }
                        });
                },),
            )
            .add_systems(Update, (debug_draw, spawn_new_tiles, despawn_old_tiles))
            .init_resource::<ExistingTilesSet>()
            .add_observer(handle_zoom_level)
            .add_observer(tile_url_to_sprite)
            .add_observer(tile_inserted)
            .add_observer(tile_replaced);
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
        self.cam.1.iter().nth(self.cam.0.0 as usize).unwrap()
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
    let x: f32 = scale.event().0;

    zoom.0 = 20; //((21.25 - x.log2()) as u8).clamp(*ZOOM_RANGE.start(), *ZOOM_RANGE.end());
    dbg!(x, x.log2(), zoom.0);

    for e in levels.iter() {
        let (level, mut tr, mut vis) = zooms.get_mut(e).unwrap();
        if level.0 == zoom.0 {
            *vis = Visibility::Inherited;
            tr.translation.z = 0.99;
        } else if level.0 == zoom.0.saturating_sub(1) {
            *vis = Visibility::Inherited;
            tr.translation.z = 0.5;
        } else if level.0 == zoom.0.saturating_add(1) {
            *vis = Visibility::Inherited;
            tr.translation.z = 0.2;
        } else {
            *vis = Visibility::Hidden;
            tr.translation.z = 0.0;
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

fn new_tile(tile: TileMathTile) -> impl Bundle {
    //let tile_coord_limit = (2 as u32).pow(tile.zoom as u32) - 1;

    let url = format!(
        "https://mapproxy.dmho.de/tms/1.0.0/thunderforest_transport/EPSG3857/{}/{}/{}.png",
        tile.zoom - 1,
        tile.x, // % (tile_coord_limit + 1),
        tile.y, //.clamp(0, tile_coord_limit)
    );
    let bbox = tile_to_aabb_world(tile);

    (
        TileUrl(url),
        Transform::from_translation(bbox.center().extend(1.0))
            .with_scale((bbox.half_size() * 2.).extend(1.0)),
        Tile(tile),
        children![(
            Text2d::new(format!("{}/{}/{}", tile.zoom, tile.x, tile.y)),
            Text2dShadow {
                offset: Vec2::new(2.0, -2.0),
                ..Default::default()
            },
            TextFont::from_font_size(100.0),
            Transform::from_scale(Vec3::ONE / 1024.).with_translation(Vec3::Z),
        )],
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
    zoom: ZoomHelper<MainCam>,
    view: ViewportConv<MainCam>,
    existing_tiles: Res<ExistingTilesSet>,
) -> Result<()> {
    let bbox = view.visible_aabb()?.world_to_tile_coords(zoom.level());
    //dbg!(bbox);
    let current_view_tiles = TileIterator::new(
        zoom.level(),
        (bbox.min.x as u32)..=(bbox.max.x as u32),
        (bbox.min.y as u32)..=(bbox.max.y as u32),
    )
    .collect::<HashSet<_>>();
    let diff = current_view_tiles.difference(&existing_tiles.0);
    //dbg!(current_view_tiles.len());
    for tile in diff {
        commands
            .entity(zoom.level_entity())
            .with_child(new_tile(*tile));
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
        .viewport_center_world()
        .unwrap()
        .world_to_tile_coords(zoom.level());
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
            let mercator_pos = pos.world_to_mercator();
            let coords = mercator_pos.mercator_to_lonlat();
            let text = format!(
                "Lat: {}, Lon: {},\n web x: {}, web y: {},\n bevy x: {}, bevy y: {}",
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
