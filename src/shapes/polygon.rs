use bevy::{asset::RenderAssetUsages, math::DVec2, mesh::Indices, prelude::*};
use lyon::{
    math::point,
    path::Path,
    tessellation::{BuffersBuilder, FillOptions, FillTessellator, VertexBuffers},
};

use crate::{MercatorCoords, shapes::utils::*};

pub(crate) fn polygon_plugin(app: &mut App) {
    app.add_systems(PostUpdate, sync_polygon_added);
}

/// A simple polygon defined by a list of points in mercator coordinates.
/// The polygon is filled with a solid color.
///
/// When adding a GeoPolygon component to an entity, a Mesh2d, MeshMaterial2d and [MercatorCoords] will be automatically created and added to the entity.
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
    query: Query<(Entity, &GeoPolygon), Changed<GeoPolygon>>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    for (entity, polygon) in query.iter() {
        debug!("Adding polygon with {} points", polygon.points.len());
        let (vertices, first_pos) = points_to_relative(&polygon.points);
        let mut path_builder = Path::builder();
        if let Some((first, rest)) = vertices.split_first() {
            path_builder.begin(point(first.x, first.y));
            rest.iter().for_each(|p| {
                path_builder.line_to(point(p.x, p.y));
            });
            path_builder.close();
        }
        let path = path_builder.build();
        let mut tessellator = FillTessellator::new();
        let mut mesh = Mesh::new(
            bevy::mesh::PrimitiveTopology::TriangleList,
            RenderAssetUsages::default(),
        );

        let fill_options =
            FillOptions::tolerance(0.1).with_fill_rule(lyon::path::FillRule::NonZero);

        let mut buffers: VertexBuffers<SimpleVertex, u32> = VertexBuffers::new();
        tessellator
            .tessellate(
                &path,
                &fill_options,
                &mut BuffersBuilder::new(&mut buffers, WithoutColor),
            )
            .unwrap();
        mesh.insert_indices(Indices::U32(buffers.indices));
        mesh.insert_attribute(
            Mesh::ATTRIBUTE_POSITION,
            buffers
                .vertices
                .iter()
                .map(|v| v.position)
                .collect::<Vec<[f32; 3]>>(),
        );

        let material = ColorMaterial {
            color: polygon.fill_color.unwrap_or(Color::WHITE),
            ..Default::default()
        };

        commands.entity(entity).insert((
            Mesh2d(meshes.add(mesh)),
            MeshMaterial2d(materials.add(material)),
            MercatorCoords(first_pos.extend(5.0)),
        ));
    }
}
