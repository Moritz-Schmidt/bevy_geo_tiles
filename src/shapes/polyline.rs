use bevy::{asset::RenderAssetUsages, math::DVec2, mesh::Indices, prelude::*};

use crate::{MercatorCoords, pancam::NewScale, shapes::utils::*};
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

/// A polyline defined by a list of points in mercator coordinates.
///
/// When adding a GeoPolyline component to an entity, a Mesh2d, MeshMaterial2d and [MercatorCoords] will be automatically created and added to the entity.
/// Set the associated [GeoPolylineConfig] to modify the appearance of the polyline.
#[derive(Component, Debug, Clone)]
#[require(GeoPolylineConfig)]
#[derive(Default)]
pub struct GeoPolyline {
    /// The points of the polyline in mercator space.
    pub points: Vec<DVec2>,
}

/// Style options for rendering a GeoPolyline.
///
/// The style can be set using the [GeoPolylineConfig] component.
///
#[derive(Debug, Clone)]
pub enum PolylineStyle {
    /// Constant width and constant color for the entire polyline.
    ConstantWidthConstantColor { width: f32, color: Color },

    /// Constant width and variable color for the entire polyline.
    /// The length of the `colors` vector should match the number of points in the polyline.
    /// a gradient will be rendered between the colors.
    ConstantWidthVariableColor { width: f32, colors: Vec<Color> },

    /// Variable width and constant color for the entire polyline.
    /// The length of the `widths` vector should match the number of points in the polyline.
    VariableWidthConstantColor { widths: Vec<f32>, color: Color },

    /// Variable width and variable color for the entire polyline.
    /// The length of the `widths` and `colors` vectors should match the number of points in the polyline.
    VariableWidthVariableColor {
        widths: Vec<f32>,
        colors: Vec<Color>,
    },
}

/// Configuration for rendering a GeoPolyline.
///
/// Use this component to set the style and appearance of a [GeoPolyline].
/// see [PolylineStyle] for available styles.
/// `start_cap`, `end_cap`, `line_join`, `miter_limit` and `tolerance` are equivalents to the Lyon [StrokeOptions] settings.
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
            tolerance: 1.0,
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
            tolerance: 1.0,
        }
    }
}

#[derive(Component, Debug, Clone)]
struct LyonPolyline {
    first_pos: DVec2,
    path: Path,
}

/// Marker component to keep the polyline width in display size regardless of zoom level.
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
        let path = match &config.style {
            PolylineStyle::ConstantWidthConstantColor { width: _, color: _ } => {
                let mut path_builder = Path::builder();
                if let Some((first, rest)) = vertices.split_first() {
                    path_builder.begin(point(first.x, first.y));
                    rest.iter().for_each(|p| {
                        path_builder.line_to(point(p.x, p.y));
                    });
                    path_builder.end(false);
                }
                path_builder.build()
            }
            PolylineStyle::ConstantWidthVariableColor { width: _, colors } => {
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
                path_builder.build()
            }
            PolylineStyle::VariableWidthConstantColor { widths, color: _ } => {
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
                path_builder.build()
            }
            PolylineStyle::VariableWidthVariableColor { widths, colors } => {
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
                path_builder.build()
            }
        };
        commands
            .entity(entity)
            .insert((LyonPolyline { first_pos, path },));
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

        let mut mesh = Mesh::new(
            bevy::mesh::PrimitiveTopology::TriangleList,
            RenderAssetUsages::default(),
        );

        let (vertices, indices, color): (Vec<[f32; 3]>, Vec<u32>, Color) = match &config.style {
            PolylineStyle::ConstantWidthConstantColor { width, color } => {
                stroke_options.line_width = *width;
                let mut buffers: VertexBuffers<SimpleVertex, u32> = VertexBuffers::new();
                tessellator
                    .tessellate(
                        &lyon_polyline.path,
                        &stroke_options,
                        &mut BuffersBuilder::new(&mut buffers, WithoutColor),
                    )
                    .unwrap();
                (
                    buffers.vertices.iter().map(|v| v.position).collect(),
                    buffers.indices,
                    *color,
                )
            }
            PolylineStyle::ConstantWidthVariableColor { width, colors: _ } => {
                stroke_options.line_width = *width;
                let mut buffers: VertexBuffers<ColorVertex, u32> = VertexBuffers::new();
                tessellator
                    .tessellate(
                        &lyon_polyline.path,
                        &stroke_options,
                        &mut BuffersBuilder::new(&mut buffers, WithColor),
                    )
                    .unwrap();
                let colors: Vec<[f32; 4]> = buffers.vertices.iter().map(|v| v.color).collect();
                mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors);
                (
                    buffers.vertices.iter().map(|v| v.position).collect(),
                    buffers.indices,
                    Color::WHITE,
                )
            }
            PolylineStyle::VariableWidthConstantColor { widths: _, color } => {
                stroke_options.variable_line_width = Some(0);
                let mut buffers: VertexBuffers<SimpleVertex, u32> = VertexBuffers::new();
                tessellator
                    .tessellate_path(
                        &lyon_polyline.path,
                        &stroke_options,
                        &mut BuffersBuilder::new(&mut buffers, WithoutColor),
                    )
                    .unwrap();
                (
                    buffers.vertices.iter().map(|v| v.position).collect(),
                    buffers.indices,
                    *color,
                )
            }
            PolylineStyle::VariableWidthVariableColor {
                widths: _,
                colors: _,
            } => {
                stroke_options.variable_line_width = Some(0);
                let mut buffers: VertexBuffers<ColorVertex, u32> = VertexBuffers::new();
                tessellator
                    .tessellate_path(
                        &lyon_polyline.path,
                        &stroke_options,
                        &mut BuffersBuilder::new(&mut buffers, WithColor),
                    )
                    .unwrap();
                let colors: Vec<[f32; 4]> = buffers.vertices.iter().map(|v| v.color).collect();
                mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors);
                (
                    buffers.vertices.iter().map(|v| v.position).collect(),
                    buffers.indices,
                    Color::WHITE,
                )
            }
        };
        mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, vertices);
        mesh.insert_indices(Indices::U32(indices));
        let material = ColorMaterial {
            color,
            ..Default::default()
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
