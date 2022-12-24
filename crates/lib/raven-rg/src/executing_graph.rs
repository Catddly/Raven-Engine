use std::borrow::Borrow;
use std::sync::Arc;

use ash::vk;
use arrayvec::ArrayVec;

use raven_rhi::{
    backend::{AccessType, CommandBuffer, Image, ImageBarrier, BufferBarrier},
    backend::barrier,
    dynamic_buffer::DynamicBuffer,
};

use crate::{
    pass::{Pass, PassResourceAccessType},
    pass_context::{PassContext, GraphResourceRegistry},
    graph_resource::{GraphResource, ExportableGraphResource, GraphResourceImportedData},
    retired_graph::RetiredRenderGraph,
    compiled_graph::{RegisteredResource, RenderGraphPipelineHandles, GraphPreparedResource, GraphPreparedResourceRef}, graph_executor::{ExecutionParams},
};

const MAX_TRANSITION_PER_BATCH: usize = 64;

pub(crate) struct ExecutingRenderGraph<'exec, 'dynamic> {
    pub(crate) execution_params: ExecutionParams<'exec>,

    pub(crate) global_dynamic_buffer: &'dynamic mut DynamicBuffer,

    pub(crate) passes: Vec<Pass>,
    pub(crate) native_resources: Vec<GraphResource>,
    pub(crate) registered_resources: Vec<RegisteredResource>,
    pub(crate) exported_resources: Vec<(ExportableGraphResource, AccessType)>,

    pub(crate) pipelines: RenderGraphPipelineHandles,
}

impl<'exec, 'dynamic> ExecutingRenderGraph<'exec, 'dynamic> {
    pub fn record_commands(
        &mut self,
        cb: &CommandBuffer,
    ) {
        let first_present_pass = self.find_first_present_pass();

        // consume all the passes and be ready for executing
        let mut passes: Vec<_> = std::mem::take(&mut self.passes).into();

        // transition all the resources to the first access type to reduce some pipeline bubbles
        {
            let mut transition_resources = Vec::new();

            for pass in &mut passes[..first_present_pass] {
                for pass_ref in pass.inputs.iter_mut().chain(pass.outputs.iter_mut()) {
                    let registered_res = self.registered_resources[pass_ref.handle.id as usize].borrow();

                    transition_resources.push((registered_res, PassResourceAccessType {
                        access_type: pass_ref.access.access_type,
                        // Force to reduce pipeline bubbles
                        skip_sync_if_same: true,
                        #[cfg(debug_assertions)]
                        debug_pass_name: pass.name.clone(),
                    }));

                    // skip when encounter this resource again!
                    pass_ref.access.skip_sync_if_same = true;
                }
            }

            for (transition_resource, access) in transition_resources {
                self.resource_transition(&cb, transition_resource, access);
            }

            // TODO: transition all resources in batched have a problem
            // when you try to transitioned same image twice in the one single barrier.
            // you will received validation error from vulkan

            // self.resource_transition_batched(&cb, transition_resources);
        }

        // record commands
        // leave only the present pass remain.
        for pass in passes.drain(..first_present_pass) {
            self.record_pass_commands(&cb, pass);
        }

        self.passes = passes.into();
    }

    pub(crate) fn record_present_commands(
        mut self,
        cb: &CommandBuffer,
        swapchain_image: Arc<Image>,
    ) -> RetiredRenderGraph {
        // in the final render pass, transition the exportable resources to the final states
        let transition_exported_resources = self.exported_resources.iter()
            .filter_map(|(export_res, access)| {
                // AccessType::Nothing here means that this resource need no transition
                if *access != AccessType::Nothing {
                    Some((&self.registered_resources[export_res.handle().id as usize], PassResourceAccessType {
                        access_type: *access,
                        skip_sync_if_same: false,
                        #[cfg(debug_assertions)]
                        debug_pass_name: String::from("final present pass"),
                    }))
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();

        self.resource_transition_batched(&cb, transition_exported_resources);

        // at this point, we can fill the GraphPreparedResource::Delayed with actual resource
        for res in self.registered_resources.iter_mut() {
            if let GraphPreparedResource::Delayed(graph_resource) = &res.resource {
                match &graph_resource {
                    GraphResource::Imported(GraphResourceImportedData::SwapchainImage) => {
                        // replace the Delayed resource with ImportedImage
                        res.resource = GraphPreparedResource::ImportedImage(swapchain_image.clone());
                    },
                    _ => panic!("For now GraphPreparedResource::Delayed must be swapchain image!"),
                }
            }
        }
        
        let passes = std::mem::take(&mut self.passes);
        for pass in passes {
            self.record_pass_commands(&cb, pass);
        }

        RetiredRenderGraph {
            registered_resources: self.registered_resources,
        }
    }

    fn find_first_present_pass(
        &self
    ) -> usize {
        let mut first_present_pass = self.passes.len();

        // find the pass that write to a swapchain image
        for (idx, pass) in self.passes.iter().enumerate() {
            for resource in &pass.outputs {
                let res = &self.native_resources[resource.handle.id as usize];

                if matches!(
                    res,
                    GraphResource::Imported(GraphResourceImportedData::SwapchainImage)
                ) {
                    first_present_pass = idx;
                    break;
                }
            }
        }

        assert_ne!(first_present_pass, self.passes.len());
        first_present_pass
    }

    fn record_pass_commands(
        &mut self,
        cb: &CommandBuffer,
        pass: Pass,
    ) {
        //glog::debug!("Recording {} pass", pass.name);
        // TODO: add pass performance ticker and debug marker!

        // transition all the pass resources to dst access
        let transition_resources = pass.inputs.iter().chain(pass.outputs.iter())
            .map(|pass_res| (&self.registered_resources[pass_res.handle.id as usize], pass_res.access.clone()))
            .collect::<Vec<_>>();

        // for (transition_resource, access) in transition_resources {
        //     self.resource_transition(&cb, transition_resource, access);
        // }
        self.resource_transition_batched(&cb, transition_resources);

        let mut context = PassContext {
            cb: cb,
            registry: GraphResourceRegistry {
                execution_params: &self.execution_params,

                pipelines: &self.pipelines,
                registered_resources: &self.registered_resources,
                global_dynamic_buffer: &mut self.global_dynamic_buffer,
            },
        };

        if let Some(callback) = pass.render_func {
            if let Err(err) = callback(&mut context) {
                panic!("Error occurs when executing pass {} with {:?}", pass.name, err);
            }
        }
    }

    #[allow(dead_code)]
    fn resource_transition(
        &self,
        cb: &CommandBuffer,
        resource: &RegisteredResource,
        target_access: PassResourceAccessType,
    ) {
        // allow pipeline to overlap
        if resource.get_current_access() == target_access.access_type && target_access.skip_sync_if_same {
            return;
        }

        let device = self.execution_params.device;

        match resource.resource.borrow() {
            GraphPreparedResourceRef::Image(image) => {
                barrier::image_barrier(device, cb.raw, &[barrier::ImageBarrier {
                    image,
	                prev_access: &[resource.get_current_access()],
	                next_access: &[target_access.access_type],
	                aspect_mask: aspect_flag_from_image_format(image.desc.format),
                    // TODO: by analyzing the lifetime of resources, we can discard the contents if possible
	                discard_contents: false,
                }]);
                
                // do NOT forget to update the access
                resource.transition_to(target_access.access_type);
            }
            GraphPreparedResourceRef::Buffer(buffer) => {
                barrier::buffer_barrier(device, cb.raw, &[barrier::BufferBarrier {
                    buffer,
	                prev_access: &[resource.get_current_access()],
	                next_access: &[target_access.access_type],
                }]);
                
                // do NOT forget to update the access
                resource.transition_to(target_access.access_type);
            }
            #[cfg(feature = "gpu_ray_tracing")]
            GraphPreparedResourceRef::RayTracingAccelStruct(_) => {
                // TODO: ray tracing memory barrier

                // barrier::buffer_barrier(device, cb.raw, &[barrier::BufferBarrier {
                //     buffer,
	            //     prev_access: &[resource.get_current_access()],
	            //     next_access: &[target_access.access_type],
                // }]);
                
                // do NOT forget to update the access
                resource.transition_to(target_access.access_type);
            }
        }
    }

    /// Use this function to transition resources if possible.
    /// Record transition command one by one is less efficient than transition them all together!
    fn resource_transition_batched<'a> (
        &self,
        cb: &CommandBuffer,
        resources: Vec<(&RegisteredResource, PassResourceAccessType)>,
    ) {
        let resource_count = resources.len();
        if resource_count == 0 {
            return;
        } 

        let batch_count = (resource_count / MAX_TRANSITION_PER_BATCH) + 1;

        for i in 0..batch_count {
            let lower_bound = i * MAX_TRANSITION_PER_BATCH;
            let upper_bound = if i == batch_count - 1 {
                resource_count
            } else {
                (i + 1) * MAX_TRANSITION_PER_BATCH
            };

            self.resource_transition_batched_impl(&cb, &resources[lower_bound..upper_bound]);
        }
    }

    fn resource_transition_batched_impl<'a> (
        &self,
        cb: &CommandBuffer,
        resources: &'a [(&'a RegisteredResource, PassResourceAccessType)],
    ) {
        if resources.len() == 1 {
            self.resource_transition(&cb, &resources[0].0, resources[0].1.clone());
            return;
        }

        // Used to avoid transition collision
        let mut need_transition: ArrayVec<bool, MAX_TRANSITION_PER_BATCH> = ArrayVec::new();

        #[cfg(debug_assertions)]
        let mut transitions: ArrayVec<(AccessType, AccessType, &String), MAX_TRANSITION_PER_BATCH> = ArrayVec::new();
        #[cfg(not(debug_assertions))]
        let mut transitions: ArrayVec<(AccessType, AccessType), MAX_TRANSITION_PER_BATCH> = ArrayVec::new();

        let mut buf_barriers: ArrayVec<BufferBarrier, MAX_TRANSITION_PER_BATCH> = ArrayVec::new();
        let mut img_barriers: ArrayVec<ImageBarrier, MAX_TRANSITION_PER_BATCH> = ArrayVec::new();

        // pre-cache all the transitions
        for (resource, access) in resources.iter() {
            // allow pipeline to overlap
            if resource.get_current_access() == access.access_type && access.skip_sync_if_same {
                need_transition.push(false); 
                continue;
            }
            need_transition.push(true);

            #[cfg(debug_assertions)]
            match resource.resource {
                // Note: pass in ref here have cons that, it can only have one argument
                GraphPreparedResource::CreatedImage(_) => { transitions.push((resource.get_current_access(), access.access_type, &access.debug_pass_name)); }
                GraphPreparedResource::ImportedImage(_) => { transitions.push((resource.get_current_access(), access.access_type, &access.debug_pass_name)); }
                
                GraphPreparedResource::CreatedBuffer(_) => { transitions.push((resource.get_current_access(), access.access_type, &access.debug_pass_name)); }
                GraphPreparedResource::ImportedBuffer(_) => { transitions.push((resource.get_current_access(), access.access_type, &access.debug_pass_name)); }

                #[cfg(feature = "gpu_ray_tracing")]
                GraphPreparedResource::ImportedRayTracingAccelStruct(_) => { transitions.push((resource.get_current_access(), access.access_type, &access.debug_pass_name)); }

                GraphPreparedResource::Delayed(_) => panic!("No transition on GraphPreparedResource::Delayed!")
            }

            #[cfg(not(debug_assertions))]
            match resource.resource {
                // Note: pass in ref here have cons that, it can only have one argument
                GraphPreparedResource::CreatedImage(_) => { transitions.push((resource.get_current_access(), access.access_type)); }
                GraphPreparedResource::ImportedImage(_) => { transitions.push((resource.get_current_access(), access.access_type)); }
                
                GraphPreparedResource::CreatedBuffer(_) => { transitions.push((resource.get_current_access(), access.access_type)); }
                GraphPreparedResource::ImportedBuffer(_) => { transitions.push((resource.get_current_access(), access.access_type)); }

                #[cfg(feature = "gpu_ray_tracing")]
                GraphPreparedResource::ImportedRayTracingAccelStruct(_) => { transitions.push((resource.get_current_access(), access.access_type)); }

                GraphPreparedResource::Delayed(_) => panic!("No transition on GraphPreparedResource::Delayed!")
            }
        }
        assert_eq!(need_transition.len(), resources.len());

        //#[cfg(debug_assertions)]
        //dbg!(&transitions);

        let mut idx = 0;
        for (res_idx, (resource, access)) in resources.iter().enumerate() {
            // if resource.get_current_access() == access.access_type && access.skip_sync_if_same {
            //     continue;
            // }
            if !need_transition[res_idx] {
                continue;
            }

            match resource.resource.borrow() {
                GraphPreparedResourceRef::Image(image) => {
                    img_barriers.push(barrier::ImageBarrier {
                        image,
                        prev_access: std::slice::from_ref(&transitions[idx].0),
                        next_access: std::slice::from_ref(&transitions[idx].1),
                        aspect_mask: aspect_flag_from_image_format(image.desc.format),
                        // TODO: by analyzing the lifetime of resources, we can discard the contents if possible
                        discard_contents: false,
                    });
                    
                    // do NOT forget to update the access
                    resource.transition_to(access.access_type);
                }
                GraphPreparedResourceRef::Buffer(buffer) => {
                    buf_barriers.push(barrier::BufferBarrier {
                        buffer,
                        prev_access: std::slice::from_ref(&transitions[idx].0),
                        next_access: std::slice::from_ref(&transitions[idx].1),
                    });
                    
                    // do NOT forget to update the access
                    resource.transition_to(access.access_type);
                }
                #[cfg(feature = "gpu_ray_tracing")]
                GraphPreparedResourceRef::RayTracingAccelStruct(_) => {
                    // TODO: ray tracing memory barrier

                    resource.transition_to(access.access_type);
                }
            }

            idx += 1;
        }

        let device = self.execution_params.device;
        // transition them all together
        if !img_barriers.is_empty() {
            barrier::image_barrier(device, cb.raw, &img_barriers);
        }
        if !buf_barriers.is_empty() {
            barrier::buffer_barrier(device, cb.raw, &buf_barriers);
        }
    }
}

// TEMPORARY: it is not the best way to get vk::ImageAspectFlags  
fn aspect_flag_from_image_format(format: vk::Format) -> vk::ImageAspectFlags {
    match format {
        vk::Format::D16_UNORM           => vk::ImageAspectFlags::DEPTH,
        vk::Format::X8_D24_UNORM_PACK32 => vk::ImageAspectFlags::DEPTH,
        vk::Format::D32_SFLOAT          => vk::ImageAspectFlags::DEPTH,
        vk::Format::S8_UINT             => vk::ImageAspectFlags::STENCIL,
        vk::Format::D16_UNORM_S8_UINT   => {
            vk::ImageAspectFlags::DEPTH | vk::ImageAspectFlags::STENCIL
        }
        vk::Format::D24_UNORM_S8_UINT   => {
            vk::ImageAspectFlags::DEPTH | vk::ImageAspectFlags::STENCIL
        }
        vk::Format::D32_SFLOAT_S8_UINT  => {
            vk::ImageAspectFlags::DEPTH | vk::ImageAspectFlags::STENCIL
        }
        _ => vk::ImageAspectFlags::COLOR,
    }
}