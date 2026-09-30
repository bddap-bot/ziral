use core::sync::atomic::Ordering;

use glow::HasContext;

use crate::AtomicFenceValue;

#[derive(Debug)]
pub struct Fence {
    last_completed: AtomicFenceValue,
}

impl crate::DynFence for Fence {}

impl Fence {
    pub fn new(_options: &wgt::GlBackendOptions) -> Self {
        Self {
            last_completed: AtomicFenceValue::new(0),
        }
    }

    pub fn signal(
        &mut self,
        context: &super::AdapterContext,
        value: crate::FenceValue,
    ) -> Result<(), crate::DeviceError> {
        // WebGL sync objects accumulate finalizers until GC. Finish the work
        // before reporting completion instead of allocating another sync.
        unsafe { context.glow_context.finish() };
        if context.webgl2_context.is_context_lost() {
            return Err(crate::DeviceError::Lost);
        }
        *self.last_completed.get_mut() = value;
        Ok(())
    }

    pub fn satisfied(&self, value: crate::FenceValue) -> bool {
        self.last_completed.load(Ordering::Acquire) >= value
    }

    pub fn get_latest(&self, _gl: &glow::Context) -> crate::FenceValue {
        self.last_completed.load(Ordering::Acquire)
    }

    pub fn maintain(&mut self, _gl: &glow::Context) {}

    pub fn wait(
        &self,
        _gl: &glow::Context,
        wait_value: crate::FenceValue,
        _timeout_ns: u32,
    ) -> Result<bool, crate::DeviceError> {
        Ok(self.satisfied(wait_value))
    }

    pub fn destroy(self, _gl: &glow::Context) {}
}
