use alloc::vec::Vec;
use glow::HasContext;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Attachment {
    pub attachment: u32,
    pub view: super::TextureView,
    pub depth_slice: Option<u32>,
    pub sample_count: u32,
}

impl Attachment {
    pub fn is_external(&self) -> bool {
        match self.view.inner {
            #[cfg(webgl)]
            super::TextureInner::ExternalFramebuffer { .. } => true,
            #[cfg(native)]
            super::TextureInner::ExternalNativeFramebuffer { .. } => true,
            _ => false,
        }
    }
}

#[derive(Default)]
pub(super) struct Cache {
    entries: Vec<(Vec<Attachment>, glow::Framebuffer)>,
}

impl Cache {
    pub unsafe fn bind(
        &mut self,
        gl: &glow::Context,
        target: u32,
        attachments: &[Attachment],
    ) -> Result<glow::Framebuffer, crate::DeviceError> {
        if let Some((_, fbo)) = self
            .entries
            .iter()
            .find(|(layout, _)| layout == attachments)
        {
            unsafe { gl.bind_framebuffer(target, Some(*fbo)) };
            return Ok(*fbo);
        }
        let fbo =
            unsafe { gl.create_framebuffer() }.map_err(|_| crate::DeviceError::OutOfMemory)?;
        unsafe { gl.bind_framebuffer(target, Some(fbo)) };
        for a in attachments {
            unsafe {
                super::Queue::set_attachment(
                    gl,
                    target,
                    a.attachment,
                    &a.view,
                    a.depth_slice,
                    a.sample_count,
                )
            };
        }
        self.entries.push((attachments.to_vec(), fbo));
        Ok(fbo)
    }

    pub unsafe fn remove(&mut self, gl: &glow::Context, texture: &super::TextureInner) {
        self.entries.retain(|(layout, fbo)| {
            if layout.iter().any(|a| &a.view.inner == texture) {
                unsafe { gl.delete_framebuffer(*fbo) };
                false
            } else {
                true
            }
        });
    }

    pub unsafe fn clear(&mut self, gl: &glow::Context) {
        for (_, fbo) in self.entries.drain(..) {
            unsafe { gl.delete_framebuffer(fbo) };
        }
    }
}
