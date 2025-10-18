use bevy::math::{DVec2, Vec2};

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
