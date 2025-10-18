use bevy::{asset::RenderAssetUsages, math::DVec2, prelude::*};

use crate::{MercatorCoords, pancam::NewScale, shapes::utils::points_to_relative};
use lyon::{
    math::point,
    path::{LineCap, LineJoin, Path},
    tessellation::{
        BuffersBuilder, StrokeOptions, StrokeTessellator, StrokeVertexConstructor, VertexBuffers,
    },
};

pub(crate) fn polyline_plugin(app: &mut App) {
    app.add_systems(PostUpdate, (sync_polyline, sync_polyline_config).chain())
        .add_observer(keep_display_width)
        .add_observer(insert_polyline_initial_style);
}

#[derive(Debug, Copy, Clone)]
struct ColorVertex {
    position: [f32; 3],
    color: [f32; 4],
}

#[derive(Debug, Copy, Clone)]
struct SimpleVertex {
    position: [f32; 3],
}

struct WithColor;
struct WithoutColor;

fn attr_to_color(attributes: &[f32]) -> Option<[f32; 4]> {
    Some([
        *attributes.get(1)?,
        *attributes.get(2)?,
        *attributes.get(3)?,
        *attributes.get(4)?,
    ])
}

impl StrokeVertexConstructor<ColorVertex> for WithColor {
    fn new_vertex(&mut self, mut vertex: lyon::tessellation::StrokeVertex) -> ColorVertex {
        let attributes = vertex.interpolated_attributes();
        let color = attr_to_color(attributes).unwrap_or([1.0, 1.0, 1.0, 1.0]);
        ColorVertex {
            position: vertex.position().extend(0.0).to_array(),
            color,
        }
    }
}

impl StrokeVertexConstructor<SimpleVertex> for WithoutColor {
    fn new_vertex(&mut self, vertex: lyon::tessellation::StrokeVertex) -> SimpleVertex {
        SimpleVertex {
            position: vertex.position().extend(0.0).to_array(),
        }
    }
}

#[derive(Component, Debug, Clone)]
#[require(GeoPolylineConfig)]
#[derive(Default)]
pub struct GeoPolyline {
    /// The points of the polyline in mercator space.
    pub points: Vec<DVec2>,
}

#[derive(Debug, Clone)]
pub enum PolylineStyle {
    ConstantWidthConstantColor {
        width: f32,
        color: Color,
    },
    ConstantWidthVariableColor {
        width: f32,
        colors: Vec<Color>,
    },
    VariableWidthConstantColor {
        widths: Vec<f32>,
        color: Color,
    },
    VariableWidthVariableColor {
        widths: Vec<f32>,
        colors: Vec<Color>,
    },
}

#[derive(Component, Debug, Clone)]
pub struct GeoPolylineConfig {
    pub style: PolylineStyle,
    pub start_cap: LineCap,
    pub end_cap: LineCap,
    pub line_join: LineJoin,
    pub miter_limit: f32,
    pub tolerance: f32,
}

#[derive(Component, Debug, Clone)]
struct PolylineInitialStyle(PolylineStyle);

impl GeoPolylineConfig {
    /// Creates a new GeoPolylineConfig with constant line width and color, and round line caps and joins.
    pub fn new(width: f32, color: Color) -> Self {
        Self {
            style: PolylineStyle::ConstantWidthConstantColor { width, color },
            start_cap: LineCap::Round,
            end_cap: LineCap::Round,
            line_join: LineJoin::Round,
            miter_limit: 4.0,
            tolerance: 0.1,
        }
    }
}

impl Default for GeoPolylineConfig {
    fn default() -> Self {
        Self {
            style: PolylineStyle::ConstantWidthConstantColor {
                width: 1.0,
                color: Color::WHITE,
            },
            start_cap: LineCap::Round,
            end_cap: LineCap::Round,
            line_join: LineJoin::Round,
            miter_limit: 4.0,
            tolerance: 0.1,
        }
    }
}

#[derive(Component, Debug, Clone)]
struct LyonPolyline {
    first_pos: DVec2,
    path: Path,
}

#[derive(Component, Debug, Clone)]
pub struct KeepDisplayWidth;

fn color_to_f32_array(color: Option<&Color>) -> [f32; 4] {
    if let Some(c) = color {
        c.to_linear().to_f32_array()
    } else {
        [1.0, 1.0, 1.0, 1.0]
    }
}

fn insert_polyline_initial_style(
    event: On<Add, GeoPolylineConfig>,
    mut commands: Commands,
    mut query: Query<&GeoPolylineConfig>,
) {
    let entity = event.entity;
    if let Ok(config) = query.get_mut(entity) {
        commands
            .entity(entity)
            .insert(PolylineInitialStyle(config.style.clone()));
    }
}

fn sync_polyline(
    query: Query<(Entity, &GeoPolyline, &GeoPolylineConfig), Changed<GeoPolylineConfig>>,
    mut commands: Commands,
) {
    for (entity, polyline, config) in query.iter() {
        let (vertices, first_pos) = points_to_relative(&polyline.points);
        match &config.style {
            PolylineStyle::ConstantWidthConstantColor { width: _, color: _ } => {
                debug!("Building ConstantWidthConstantColor polyline");
                let mut path_builder = Path::builder();
                if let Some((first, rest)) = vertices.split_first() {
                    path_builder.begin(point(first.x, first.y));
                    rest.iter().for_each(|p| {
                        path_builder.line_to(point(p.x, p.y));
                    });
                    path_builder.end(false);
                }
                let path = path_builder.build();
                commands
                    .entity(entity)
                    .insert((LyonPolyline { first_pos, path },));
            }
            PolylineStyle::ConstantWidthVariableColor { width: _, colors } => {
                debug!("Building ConstantWidthVariableColor polyline");
                let mut path_builder = Path::builder_with_attributes(4);
                if let Some((first, rest)) = vertices.split_first() {
                    let color = color_to_f32_array(colors.first());
                    path_builder.begin(point(first.x, first.y), &color);
                    rest.iter().enumerate().for_each(|(i, p)| {
                        let color = color_to_f32_array(colors.get(i + 1));
                        path_builder.line_to(point(p.x, p.y), &color);
                    });
                    path_builder.end(false);
                }
                let path = path_builder.build();
                commands
                    .entity(entity)
                    .insert((LyonPolyline { first_pos, path },));
            }
            PolylineStyle::VariableWidthConstantColor { widths, color: _ } => {
                debug!("Building VariableWidthConstantColor polyline");
                let mut path_builder = Path::builder_with_attributes(1);
                if let Some((first, rest)) = vertices.split_first() {
                    let width = widths.first().cloned().unwrap_or(1.0f32);
                    path_builder.begin(point(first.x, first.y), &[width]);
                    rest.iter().enumerate().for_each(|(i, p)| {
                        let width = widths.get(i + 1).cloned().unwrap_or(1.0f32);
                        path_builder.line_to(point(p.x, p.y), &[width]);
                    });
                    path_builder.end(false);
                }
                let path = path_builder.build();
                commands
                    .entity(entity)
                    .insert((LyonPolyline { first_pos, path },));
            }
            PolylineStyle::VariableWidthVariableColor { widths, colors } => {
                debug!("Building VariableWidthVariableColor polyline");
                let mut path_builder = Path::builder_with_attributes(5);
                if let Some((first, rest)) = vertices.split_first() {
                    let width = widths.first().cloned().unwrap_or(1.0f32);
                    let color = color_to_f32_array(colors.first());
                    let attributes = [width, color[0], color[1], color[2], color[3]];
                    path_builder.begin(point(first.x, first.y), &attributes);
                    rest.iter().enumerate().for_each(|(i, p)| {
                        let width = widths.get(i + 1).cloned().unwrap_or(1.0f32);
                        let color = color_to_f32_array(colors.get(i + 1));
                        let attributes = [width, color[0], color[1], color[2], color[3]];
                        path_builder.line_to(point(p.x, p.y), &attributes);
                    });
                    path_builder.end(false);
                }
                let path = path_builder.build();
                commands
                    .entity(entity)
                    .insert((LyonPolyline { first_pos, path },));
            }
        }
    }
}

fn sync_polyline_config(
    query: Query<(Entity, &GeoPolylineConfig, &LyonPolyline), Changed<GeoPolylineConfig>>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    for (entity, config, lyon_polyline) in query.iter() {
        let mut stroke_options = StrokeOptions::default()
            .with_start_cap(config.start_cap)
            .with_end_cap(config.end_cap)
            .with_line_join(config.line_join)
            .with_miter_limit(config.miter_limit)
            .with_tolerance(config.tolerance);
        let mut tessellator = StrokeTessellator::new();
        let (mesh, material) = match &config.style {
            PolylineStyle::ConstantWidthConstantColor { width, color } => {
                debug!("Tessellating ConstantWidthConstantColor polyline");
                stroke_options.line_width = *width;
                let mut buffers: VertexBuffers<SimpleVertex, u32> = VertexBuffers::new();
                tessellator
                    .tessellate(
                        &lyon_polyline.path,
                        &stroke_options,
                        &mut BuffersBuilder::new(&mut buffers, WithoutColor),
                    )
                    .unwrap();

                let mut mesh = Mesh::new(
                    bevy::mesh::PrimitiveTopology::TriangleList,
                    RenderAssetUsages::default(),
                );
                let positions: Vec<[f32; 3]> =
                    buffers.vertices.iter().map(|v| v.position).collect();
                mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
                mesh.insert_indices(bevy::mesh::Indices::U32(buffers.indices));

                let material = ColorMaterial {
                    color: *color,
                    ..Default::default()
                };

                (mesh, material)
            }
            PolylineStyle::ConstantWidthVariableColor { width, colors: _ } => {
                debug!("Tessellating ConstantWidthVariableColor polyline");
                stroke_options.line_width = *width;
                let mut buffers: VertexBuffers<ColorVertex, u32> = VertexBuffers::new();
                tessellator
                    .tessellate(
                        &lyon_polyline.path,
                        &stroke_options,
                        &mut BuffersBuilder::new(&mut buffers, WithColor),
                    )
                    .unwrap();

                let mut mesh = Mesh::new(
                    bevy::mesh::PrimitiveTopology::TriangleList,
                    RenderAssetUsages::default(),
                );
                let positions: Vec<[f32; 3]> =
                    buffers.vertices.iter().map(|v| v.position).collect();
                mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
                mesh.insert_indices(bevy::mesh::Indices::U32(buffers.indices));
                let colors: Vec<[f32; 4]> = buffers.vertices.iter().map(|v| v.color).collect();
                mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors);

                (mesh, ColorMaterial::default())
            }
            PolylineStyle::VariableWidthConstantColor { widths: _, color } => {
                debug!("Tessellating VariableWidthConstantColor polyline");
                stroke_options.variable_line_width = Some(0);
                let mut buffers: VertexBuffers<SimpleVertex, u32> = VertexBuffers::new();
                tessellator
                    .tessellate_path(
                        &lyon_polyline.path,
                        &stroke_options,
                        &mut BuffersBuilder::new(&mut buffers, WithoutColor),
                    )
                    .unwrap();

                let mut mesh = Mesh::new(
                    bevy::mesh::PrimitiveTopology::TriangleList,
                    RenderAssetUsages::default(),
                );
                let positions: Vec<[f32; 3]> =
                    buffers.vertices.iter().map(|v| v.position).collect();
                mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
                mesh.insert_indices(bevy::mesh::Indices::U32(buffers.indices));

                let material = ColorMaterial {
                    color: *color,
                    ..Default::default()
                };

                (mesh, material)
            }
            PolylineStyle::VariableWidthVariableColor {
                widths: _,
                colors: _,
            } => {
                debug!("Tessellating VariableWidthVariableColor polyline");
                stroke_options.variable_line_width = Some(0);
                let mut buffers: VertexBuffers<ColorVertex, u32> = VertexBuffers::new();
                tessellator
                    .tessellate_path(
                        &lyon_polyline.path,
                        &stroke_options,
                        &mut BuffersBuilder::new(&mut buffers, WithColor),
                    )
                    .unwrap();

                let mut mesh = Mesh::new(
                    bevy::mesh::PrimitiveTopology::TriangleList,
                    RenderAssetUsages::default(),
                );
                let positions: Vec<[f32; 3]> = buffers
                    .vertices
                    .iter()
                    .map(|v| [v.position[0], v.position[1], 0.0])
                    .collect();
                mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
                mesh.insert_indices(bevy::mesh::Indices::U32(buffers.indices));
                let colors: Vec<[f32; 4]> = buffers.vertices.iter().map(|v| v.color).collect();
                mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors);

                (mesh, ColorMaterial::default())
            }
        };
        commands.entity(entity).insert((
            Mesh2d(meshes.add(mesh)),
            MeshMaterial2d(materials.add(material)),
            MercatorCoords(lyon_polyline.first_pos.extend(5.0)),
        ));
    }
}

fn keep_display_width(
    scale: On<NewScale>,
    mut query: Query<(&mut GeoPolylineConfig, &PolylineInitialStyle), With<KeepDisplayWidth>>,
) {
    let scale = scale.0;
    for (mut config, initial_config) in query.iter_mut() {
        match &mut config.style {
            PolylineStyle::ConstantWidthConstantColor { width, color: _ }
            | PolylineStyle::ConstantWidthVariableColor { width, colors: _ } => {
                *width = match initial_config.0 {
                    PolylineStyle::ConstantWidthConstantColor { width, color: _ }
                    | PolylineStyle::ConstantWidthVariableColor { width, colors: _ } => {
                        width * scale * 0.1
                    }
                    _ => 1.0,
                };
            }
            PolylineStyle::VariableWidthConstantColor { widths, color: _ }
            | PolylineStyle::VariableWidthVariableColor { widths, colors: _ } => {
                *widths = match &initial_config.0 {
                    PolylineStyle::VariableWidthConstantColor { widths, color: _ }
                    | PolylineStyle::VariableWidthVariableColor { widths, colors: _ } => widths
                        .iter()
                        .map(|w| w * scale * 0.01)
                        .collect::<Vec<f32>>(),
                    _ => vec![],
                };
            }
        }
        config.tolerance = (scale * 0.001).max(0.0001); // TODO: maybe adjust this further? -> needs (performance) testing
    }
}
