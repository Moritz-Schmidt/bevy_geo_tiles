use bevy::{prelude::*, sprite::Anchor};

use bevy_geo_tiles::MapPlugin;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(MapPlugin {
            initial_zoom: 14,
            initial_center: (13.4064, 52.51977).into(),
            tile_source: "https://tile.openstreetmap.org/{z}/{x}/{y}.png".to_string(),
            ..Default::default()
        })
        .add_systems(Startup, add_marker)
        .run();
}

fn add_marker(mut commands: Commands, asset_server: Res<AssetServer>) {
    let marker = asset_server.load("examples/marker.png");

    commands.spawn((
        Sprite::from_image(marker.clone()),
        Anchor::BOTTOM_CENTER,
        bevy_geo_tiles::MercatorCoords::from_latlon(52.51977, 13.4064).with_z(5.0),
        bevy_geo_tiles::KeepDisplaySize,
    ));
}
