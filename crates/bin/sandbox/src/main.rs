extern crate log as glog;

use std::sync::Arc;

use raven_engine::prelude::{*,
    input::{KeyCode, VirtualKeyCode, InputBinding},
    asset::{AssetLoadDesc, AssetHandle},
    math::{Affine3A, Vec3, Quat},
    scene::camera
};

pub struct Sandbox;

impl Sandbox {
    pub fn load_assets(&self) -> Vec<Arc<AssetHandle>> {
        let asset_api = asset::get().read();

        asset_api.load_asset(AssetLoadDesc::load_mesh("mesh/cerberus_gun/scene.gltf")).unwrap();
        //asset_manager.load_asset(AssetLoadDesc::load_mesh("mesh/cornell_box/scene.gltf")).unwrap();
        asset_api.load_asset(AssetLoadDesc::load_texture("texture/skybox/right.jpg")).unwrap();
        asset_api.load_asset(AssetLoadDesc::load_texture("texture/skybox/left.jpg")).unwrap();
        asset_api.load_asset(AssetLoadDesc::load_texture("texture/skybox/top.jpg")).unwrap();
        asset_api.load_asset(AssetLoadDesc::load_texture("texture/skybox/bottom.jpg")).unwrap();
        asset_api.load_asset(AssetLoadDesc::load_texture("texture/skybox/front.jpg")).unwrap();
        asset_api.load_asset(AssetLoadDesc::load_texture("texture/skybox/back.jpg")).unwrap();

        asset_api.dispatch_load_tasks().unwrap()
    }
    
    pub fn prepare_scene(&self, tex_handles: &[Arc<AssetHandle>; 6], mesh_handle: &Arc<AssetHandle>) {
        let mut render_api = render::get().write(); 
        render_api.add_cubemap_split(tex_handles);
        let mesh_handle = render_api.add_mesh(mesh_handle);

        let gun_xform = Affine3A::from_scale_rotation_translation(
            Vec3::splat(0.05),
            Quat::from_rotation_y(90_f32.to_radians()),
            Vec3::splat(0.0)
        );
        // let cornell_xform = Affine3A::from_scale_rotation_translation(
        //     Vec3::splat(1.0),
        //     Quat::IDENTITY,
        //     Vec3::splat(0.0)
        // );

        let _instance = render_api.add_mesh_instance(mesh_handle, gun_xform);

        let resolution = render_api.get_render_resolution();
        let camera = camera::Camera::builder()
            .aspect_ratio(resolution[0] as f32 / resolution[1] as f32)
            .build();
        let camera_controller = camera::controller::FirstPersonController::new(Vec3::new(0.0, 0.5, 5.0), Quat::IDENTITY);

        render_api.set_main_camera(camera, camera_controller);
    }

    pub fn bind_input(&self) {
        let mut input_api = input::get().write();

        input_api.add_binding(
            KeyCode::vkcode(VirtualKeyCode::W), 
            InputBinding::new("walk", 1.0).activation_time(0.2)
        );
        input_api.add_binding(
            KeyCode::vkcode(VirtualKeyCode::S), 
            InputBinding::new("walk", -1.0).activation_time(0.2)
        );
        input_api.add_binding(
            KeyCode::vkcode(VirtualKeyCode::A), 
            InputBinding::new("strafe", -1.0).activation_time(0.2)
        );
        input_api.add_binding(
            KeyCode::vkcode(VirtualKeyCode::D), 
            InputBinding::new("strafe", 1.0).activation_time(0.2)
        );
        input_api.add_binding(
            KeyCode::vkcode(VirtualKeyCode::Q), 
            InputBinding::new("lift", -1.0).activation_time(0.2)
        );
        input_api.add_binding(
            KeyCode::vkcode(VirtualKeyCode::E), 
            InputBinding::new("lift", 1.0).activation_time(0.2)
        );
    }
}

impl App for Sandbox {
    fn init(&mut self) -> anyhow::Result<()> {
        glog::info!("User app init!");

        let handles = self.load_assets();
        let tex_handles: &[Arc<AssetHandle>; 6] = handles.split_at(1).1.try_into().unwrap();
        
        self.prepare_scene(tex_handles, &handles[0]);
        self.bind_input();

        Ok(())
    }

    fn tick_logic(&mut self, _dt: f32) {
        let input_api = input::get();
        let res = input_api.read().is_keyboard_just_pressed(input::VirtualKeyCode::P);

        if res {
            glog::debug!("P is pressed!");
        }
    }

    fn shutdown(&mut self) {
        glog::info!("User app shutdown!");
    }
}

raven_main!{ Sandbox }