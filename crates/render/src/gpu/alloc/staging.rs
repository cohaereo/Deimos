use parking_lot::{Mutex, MutexGuard};

/// Thread-safe staging buffer for uploading data to the GPU once
///
/// Mainly used to keep individual allocations aligned to the GPU's constant buffer requirements.
pub struct ImmutableStaging {
    data: Mutex<Vec<u8>>,
}

impl ImmutableStaging {
    const BLOCK_ALIGNMENT: usize = 256;

    pub fn new(capacity: usize) -> Self {
        Self {
            data: Mutex::new(Vec::with_capacity(capacity)),
        }
    }

    pub fn upload_slice(&self, data: &[u8]) -> usize {
        let size_aligned = data.len().next_multiple_of(Self::BLOCK_ALIGNMENT);
        let mut buffer = self.data.lock();

        assert!(
            buffer.len().is_multiple_of(Self::BLOCK_ALIGNMENT),
            "Misaligned upload buffer"
        );

        let start = buffer.len();
        buffer.resize(start + size_aligned, 0);
        buffer[start..start + data.len()].copy_from_slice(data);
        start
    }

    pub fn data(&self) -> MutexGuard<'_, Vec<u8>> {
        self.data.lock()
    }

    pub fn into_inner(self) -> Vec<u8> {
        self.data.into_inner()
    }
}
