use bevy::{asset::RenderAssetUsages, math::DVec2, prelude::*};
use earcut::Earcut;

use crate::{LocalOrigin, LocalOriginConversion, LocalOriginUpdated, MercatorCoords};

pub(crate) fn polygon_plugin(app: &mut App) {
    app.add_systems(PostUpdate, (sync_polygon_added, sync_polygon_changed))
        .add_observer(update_polygon_on_origin_change);
}

#[derive(Component, Debug, Clone)]
#[require(GeoPolygon)]
pub struct GeoPolygonOutline {
    pub color: Color,
    pub width: f32,
    pub joints: GizmoLineJoint,
    pub style: GizmoLineStyle,
}

#[derive(Component, Debug, Clone)]
pub struct GeoPolygon {
    pub points: Vec<DVec2>,
    pub fill_color: Option<Color>,
}

impl Default for GeoPolygon {
    fn default() -> Self {
        Self {
            points: Vec::new(),
            fill_color: Some(Color::WHITE),
        }
    }
}

fn sync_polygon_added(
    query: Query<
        (Entity, &GeoPolygon, Option<&GeoPolygonOutline>),
        (Added<GeoPolygon>, Without<Gizmo>),
    >,
    mut commands: Commands,
    origin: Res<LocalOrigin>,
    mut gizmo_assets: ResMut<Assets<GizmoAsset>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    for (entity, polygon, outline) in query.iter() {
        debug!("Adding polygon with {} points", polygon.points.len());
        let first = match polygon.points.first() {
            Some(p) => p,
            None => {
                error!("Polygon has no points");
                continue;
            }
        };
        let first_merc = first.mercator_to_local(&origin);
        let vertices: Vec<[f32; 2]> = polygon
            .points
            .iter()
            .map(|p| {
                let local_pos = p.mercator_to_local(&origin) - first_merc;
                [local_pos.x as f32, local_pos.y as f32]
            })
            .collect();
        dbg!(&vertices);
        let mut earcut = Earcut::new();
        let mut triangles: Vec<u32> = Vec::with_capacity(vertices.len() * 3);
        earcut.earcut(vertices.clone(), &[], &mut triangles);
        if let Some(color) = polygon.fill_color {
            let mut mesh = Mesh::new(
                bevy::mesh::PrimitiveTopology::TriangleList,
                RenderAssetUsages::default(),
            );
            let positions: Vec<[f32; 3]> = vertices.iter().map(|v| [v[0], v[1], 0.0]).collect();
            mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
            mesh.insert_indices(bevy::mesh::Indices::U32(triangles));
            commands.entity(entity).insert((
                Mesh2d(meshes.add(mesh)),
                MeshMaterial2d(materials.add(ColorMaterial {
                    color,
                    ..Default::default()
                })),
                MercatorCoords(first.extend(5.0)),
            ));
        }
        if let Some(outline) = outline {
            debug!("Polygon with outline (WIP)");
        }
    }
}

fn sync_polygon_changed(
    mut query: Query<(&GeoPolygon, Option<&GeoPolygonOutline>, &mut Gizmo), Changed<GeoPolygon>>,
    origin: Res<LocalOrigin>,
    mut gizmo_assets: ResMut<Assets<GizmoAsset>>,
) {
    for (polygon, outline, mut gizmo) in query.iter_mut() {
        let vertices: Vec<Vec2> = polygon
            .points
            .iter()
            .map(|p| {
                let local_pos = p.mercator_to_local(&origin);
                Vec2::new(local_pos.x as f32, local_pos.y as f32)
            })
            .collect();
        if let Some(color) = polygon.fill_color {
            todo!("Changing filled polygons is not yet supported");
        }
        if let Some(outline) = outline {
            todo!("Changing polygon outlines is not yet supported");
        }
    }
}

fn update_polygon_on_origin_change(
    _event: On<LocalOriginUpdated>,
    query: Query<(&GeoPolygon, Option<&GeoPolygonOutline>, &Gizmo)>,
    mut gizmo_assets: ResMut<Assets<GizmoAsset>>,
    origin: Res<LocalOrigin>,
) {
    debug!("Updating polygons on origin change");
    query.iter().for_each(|(polygon, outline, gizmo)| {
        if let Some(gizmo) = gizmo_assets.get_mut(&gizmo.handle) {
            let vertices: Vec<Vec2> = polygon
                .points
                .iter()
                .map(|p| {
                    let local_pos = p.mercator_to_local(&origin);
                    Vec2::new(local_pos.x as f32, local_pos.y as f32)
                })
                .collect();
            if let Some(color) = polygon.fill_color {
                todo!("Changing filled polygons on origin change is not yet supported");
            }
            if let Some(outline) = outline {
                todo!("Changing polygon outlines on origin change is not yet supported");
            }
        }
    });
}
