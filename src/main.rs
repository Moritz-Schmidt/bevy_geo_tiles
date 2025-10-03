use bevy::{log::LogPlugin, prelude::*};
use bevy_pancam::{PanCam, PanCamPlugin};
use bevy::dev_tools::fps_overlay::FpsOverlayPlugin;
use bevy_geo_tiles::MapPlugin;


fn setup(mut commands: Commands) {
    commands.spawn((Camera2d, PanCam::default()));
}

fn main() {
    App::new()
        // Configure settings with defaults
        .add_plugins(DefaultPlugins.set(LogPlugin {
            filter: "info,wgpu_core=warn,wgpu_hal=warn,naga=warn,ureq=debug,bevy_asset=debug".into(),
            level: bevy::log::Level::DEBUG,
            ..Default::default()
        }).set(AssetPlugin {
            meta_check: bevy::asset::AssetMetaCheck::Never,
            ..Default::default()
        }))
        .add_plugins(PanCamPlugin::default())
        .add_plugins(FpsOverlayPlugin::default())
        .add_plugins(MapPlugin)
        .add_systems(Startup, setup)
        .run();
}