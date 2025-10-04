use bevy::dev_tools::fps_overlay::FpsOverlayPlugin;
use bevy::{log::LogPlugin, prelude::*};
use bevy_geo_tiles::MapPlugin;

fn main() {
    App::new()
        // Configure settings with defaults
        .add_plugins(
            DefaultPlugins
                .set(LogPlugin {
                    filter:
                        "info,wgpu_core=warn,wgpu_hal=warn,naga=warn,ureq=info,bevy_asset=debug,bevy_geo_tiles=debug"
                            .into(),
                    level: bevy::log::Level::DEBUG,
                    ..Default::default()
                })
                .set(AssetPlugin {
                    meta_check: bevy::asset::AssetMetaCheck::Never,
                    ..Default::default()
                }),
        )
        .add_plugins(FpsOverlayPlugin::default())
        .add_plugins(MapPlugin {
            initial_zoom: 5,
            initial_center: (13.4064, 52.51977).into(),
        })
        .run();
}
