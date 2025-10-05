use bevy::{input::gestures::PinchGesture, prelude::*};

pub(crate) fn pancam_plugin(app: &mut App) {
    app.add_systems(Startup, setup)
        .add_systems(Update, (pinch_zoom, zoom_smooth).chain());
}

#[derive(Component, Debug)]
pub struct MainCam;

fn setup(mut commands: Commands, window: Single<Entity, With<Window>>) {
    commands.entity(*window).observe(camera_drag).observe(zoom);
}

fn camera_drag(
    drag: On<Pointer<Drag>>,
    mut cam: Single<(&Camera, &GlobalTransform, &mut Transform)>,
) -> Result {
    let mut cam_viewport = cam.0.world_to_viewport(cam.1, cam.2.translation)?;
    cam_viewport += drag.delta * -1.; // inverted feels more natural
    cam.2.translation = cam.0.viewport_to_world_2d(cam.1, cam_viewport)?.extend(0.0);
    Ok(())
}

#[derive(Component, Debug)]
pub(crate) struct SmoothZoom {
    pub(crate) target_scale: f32,
}
impl Default for SmoothZoom {
    fn default() -> Self {
        Self { target_scale: 1.0 }
    }
}

fn zoom(scroll: On<Pointer<Scroll>>, mut zoom: Single<&mut SmoothZoom, With<Camera>>) {
    let speed = match scroll.unit {
        bevy::input::mouse::MouseScrollUnit::Line => 0.1,
        bevy::input::mouse::MouseScrollUnit::Pixel => 0.02,
    };
    zoom.target_scale *= 1.0 - (scroll.y * speed);
}

fn pinch_zoom(
    mut pinch: MessageReader<PinchGesture>,
    mut zoom: Single<&mut SmoothZoom, With<Camera>>,
) {
    for p in pinch.read() {
        zoom.target_scale *= 1.0 - (p.0)
    }
}

#[derive(Event, Debug)]
pub(crate) struct NewScale(pub f32);

fn zoom_smooth(
    mut commands: Commands,
    cam: Single<(&mut Projection, &mut SmoothZoom), With<Camera>>,
    time: Res<Time>,
) {
    let (proj, zoom) = cam.into_inner();
    if let Projection::Orthographic(ref mut proj) = *proj.into_inner() {
        let mut new_scale = proj.scale;
        new_scale.smooth_nudge(&zoom.target_scale, 20., time.delta_secs());
        if (proj.scale - new_scale).abs() < 0.001 {
            return;
        }
        proj.scale = new_scale;
        commands.trigger(NewScale(new_scale));
    }
}
