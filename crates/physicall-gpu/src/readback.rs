use std::sync::Arc;
use std::sync::atomic::{AtomicU8, Ordering};

const IDLE: u8 = 0;
const RECORDED: u8 = 1;
const PENDING: u8 = 2;
const READY: u8 = 3;

/// Copies GPU buffers into one mappable staging buffer and maps it without
/// blocking, so a frame loop can read results a frame or two later.
pub struct Readback {
    buffer: wgpu::Buffer,
    state: Arc<AtomicU8>,
}

impl Readback {
    pub fn new(device: &wgpu::Device, size: u64) -> Self {
        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("readback"),
            size: size.max(16),
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        Self {
            buffer,
            state: Arc::new(AtomicU8::new(IDLE)),
        }
    }

    /// Records whole-buffer copies of `sources`, back to back, unless a read
    /// is already in flight. Returns whether it recorded.
    pub fn record(&self, encoder: &mut wgpu::CommandEncoder, sources: &[&wgpu::Buffer]) -> bool {
        if self.state.load(Ordering::Acquire) != IDLE {
            return false;
        }
        let mut offset = 0;
        for source in sources {
            encoder.copy_buffer_to_buffer(source, 0, &self.buffer, offset, source.size());
            offset += source.size();
        }
        self.state.store(RECORDED, Ordering::Release);
        true
    }

    /// Starts mapping. Call after submitting the encoder passed to `record`.
    pub fn map(&self) {
        if self.state.load(Ordering::Acquire) != RECORDED {
            return;
        }
        self.state.store(PENDING, Ordering::Release);
        let state = Arc::clone(&self.state);
        self.buffer
            .map_async(wgpu::MapMode::Read, .., move |result| {
                state.store(if result.is_ok() { READY } else { IDLE }, Ordering::Release);
            });
    }

    /// The copied bytes, once mapping has finished.
    pub fn take(&self) -> Option<Vec<u8>> {
        if self.state.load(Ordering::Acquire) != READY {
            return None;
        }
        let bytes = self
            .buffer
            .get_mapped_range(..)
            .ok()
            .map(|view| view.to_vec());
        self.buffer.unmap();
        self.state.store(IDLE, Ordering::Release);
        bytes
    }
}

/// Reads buffers synchronously. Native only: on the web, mapping completes
/// from the browser's event loop.
#[cfg(not(target_arch = "wasm32"))]
pub fn read_buffers(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    sources: &[&wgpu::Buffer],
) -> Vec<u8> {
    let readback = Readback::new(device, sources.iter().map(|b| b.size()).sum());
    let mut encoder = device.create_command_encoder(&Default::default());
    readback.record(&mut encoder, sources);
    queue.submit([encoder.finish()]);
    readback.map();
    // A failed wait leaves the buffer unmapped, which `take` reports as empty.
    let _ = device.poll(wgpu::PollType::wait_indefinitely());
    readback.take().unwrap_or_default()
}
