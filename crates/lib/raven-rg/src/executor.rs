use std::sync::Arc;

use raven_rhi::{RHI, backend::Device};

use super::graph::CainozoicRenderGraph;

/// Render graph executor to build and run a render graph with RHI.
pub struct Executor {
    device: Arc<Device>,
}

impl Executor {
    pub fn new(rhi: &RHI) -> anyhow::Result<Self> {
        Ok(Self {
            device: rhi.device.clone(),
        })
    }

    pub fn prepare<PrepareFunc>(
        &mut self,
        prepare_func: PrepareFunc,
    ) -> anyhow::Result<()>
    where
        PrepareFunc: FnOnce(&mut CainozoicRenderGraph)
    {
        let mut cainozoic_rg = CainozoicRenderGraph::new(self.device.clone());

        // user-side callback
        prepare_func(&mut cainozoic_rg);

        let (rg, exported_temp_resources) = cainozoic_rg.export_all_imported_resources();

        Ok(())
    }
}
