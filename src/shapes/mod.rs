pub mod polygon;
pub mod polyline;
use bevy::prelude::*;
mod utils;

use crate::shapes::polygon::polygon_plugin;
use crate::shapes::polyline::polyline_plugin;

pub(crate) fn shapes_plugin(app: &mut App) {
    app.add_plugins((polygon_plugin, polyline_plugin));
}
