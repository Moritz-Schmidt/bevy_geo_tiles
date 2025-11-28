use bevy::prelude::*;

use bevy_geo_tiles::MapPlugin;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(MapPlugin {
            initial_zoom: 10,
            initial_center: (13.4064, 52.51977).into(),
            tile_source: "https://tile.openstreetmap.org/{z}/{x}/{y}.png".to_string(),
            ..Default::default()
        })
        .run();
}
