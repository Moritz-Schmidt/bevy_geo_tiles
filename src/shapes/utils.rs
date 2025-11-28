use bevy::log::tracing::instrument;
use bevy::math::{DVec2, Vec2};

use lyon::tessellation::{FillVertexConstructor, StrokeVertexConstructor};

#[instrument]
pub(crate) fn points_to_relative(points: &Vec<DVec2>) -> (Vec<Vec2>, DVec2) {
    if points.is_empty() {
        return (vec![], DVec2::ZERO);
    }
    let first = points[0];
    (
        points
            .iter()
            .map(|p| (p - first).as_vec2())
            .collect::<Vec<Vec2>>(),
        first,
    )
}

#[derive(Debug, Copy, Clone)]
pub(crate) struct ColorVertex {
    pub(crate) position: [f32; 3],
    pub(crate) color: [f32; 4],
}

#[derive(Debug, Copy, Clone)]
pub(crate) struct SimpleVertex {
    pub(crate) position: [f32; 3],
}

pub(crate) struct WithColor;
pub(crate) struct WithoutColor;

pub(crate) fn attr_to_color(attributes: &[f32]) -> Option<[f32; 4]> {
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

impl FillVertexConstructor<SimpleVertex> for WithoutColor {
    fn new_vertex(&mut self, vertex: lyon::tessellation::FillVertex) -> SimpleVertex {
        SimpleVertex {
            position: vertex.position().extend(0.0).to_array(),
        }
    }
}
