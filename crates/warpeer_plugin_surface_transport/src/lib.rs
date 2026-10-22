#[cfg(target_os = "macos")]
mod apple {
    use std::slice;
    use std::sync::atomic::{AtomicU64, Ordering};

    use objc2_core_foundation::{CFBoolean, CFDictionary, CFNumber, CFType};
    #[allow(deprecated)]
    use objc2_io_surface::{
        IOSurfaceLockOptions, IOSurfaceRef, kIOSurfaceBytesPerElement, kIOSurfaceHeight,
        kIOSurfaceIsGlobal, kIOSurfacePixelFormat, kIOSurfaceWidth,
    };

    const BGRA_PIXEL_FORMAT: i32 = i32::from_be_bytes(*b"BGRA");
    static LIVE_SURFACE_BYTES: AtomicU64 = AtomicU64::new(0);

    pub struct ProducerSurface {
        surface: objc2_core_foundation::CFRetained<IOSurfaceRef>,
        width: u32,
        height: u32,
        allocation_bytes: u64,
    }

    impl ProducerSurface {
        #[allow(deprecated)]
        pub fn new(width: u32, height: u32) -> Result<Self, SurfaceTransportError> {
            if width == 0 || height == 0 {
                return Err(SurfaceTransportError::InvalidDimensions);
            }
            let width_value = CFNumber::new_i64(width.into());
            let height_value = CFNumber::new_i64(height.into());
            let bytes_per_element = CFNumber::new_i32(4);
            let pixel_format = CFNumber::new_i32(BGRA_PIXEL_FORMAT);
            let keys = unsafe {
                [
                    kIOSurfaceWidth.as_ref(),
                    kIOSurfaceHeight.as_ref(),
                    kIOSurfaceBytesPerElement.as_ref(),
                    kIOSurfacePixelFormat.as_ref(),
                    kIOSurfaceIsGlobal.as_ref(),
                ]
            };
            // The sidecar protocol currently transfers the IOSurface ID over its bounded IPC
            // channel. Keep this deprecated property isolated here until that token becomes a
            // Mach-port or XPC object; PocketJS code never receives an IOSurface API handle.
            let properties = CFDictionary::<CFType, CFType>::from_slices(
                &keys,
                &[
                    width_value.as_ref(),
                    height_value.as_ref(),
                    bytes_per_element.as_ref(),
                    pixel_format.as_ref(),
                    CFBoolean::new(true).as_ref(),
                ],
            );
            let properties: &CFDictionary = (&*properties).as_ref();
            let surface = unsafe { IOSurfaceRef::new(properties) }
                .ok_or(SurfaceTransportError::CreationFailed)?;
            let allocation_bytes = surface.bytes_per_row().saturating_mul(surface.height()) as u64;
            LIVE_SURFACE_BYTES.fetch_add(allocation_bytes, Ordering::AcqRel);
            Ok(Self {
                surface,
                width,
                height,
                allocation_bytes,
            })
        }

        pub fn matches(&self, width: u32, height: u32) -> bool {
            self.width == width && self.height == height
        }

        pub fn allocation_bytes(&self) -> u64 {
            self.allocation_bytes
        }

        pub fn write_bgra(
            &self,
            pixels: &[u8],
            source_stride: u32,
        ) -> Result<SurfaceToken, SurfaceTransportError> {
            let row_bytes = self.width as usize * 4;
            let source_stride = source_stride as usize;
            if source_stride < row_bytes
                || pixels.len() < source_stride.saturating_mul(self.height as usize)
            {
                return Err(SurfaceTransportError::InvalidBuffer);
            }
            let mut seed = 0;
            if unsafe { self.surface.lock(IOSurfaceLockOptions::empty(), &mut seed) } != 0 {
                return Err(SurfaceTransportError::LockFailed);
            }
            let destination_stride = self.surface.bytes_per_row();
            let destination = unsafe {
                slice::from_raw_parts_mut(
                    self.surface.base_address().as_ptr().cast::<u8>(),
                    destination_stride * self.height as usize,
                )
            };
            for row in 0..self.height as usize {
                let source_start = row * source_stride;
                let destination_start = row * destination_stride;
                destination[destination_start..destination_start + row_bytes]
                    .copy_from_slice(&pixels[source_start..source_start + row_bytes]);
            }
            if unsafe {
                self.surface
                    .unlock(IOSurfaceLockOptions::empty(), &mut seed)
            } != 0
            {
                return Err(SurfaceTransportError::UnlockFailed);
            }
            Ok(SurfaceToken {
                surface_id: self.surface.id(),
                seed,
            })
        }
    }

    impl Drop for ProducerSurface {
        fn drop(&mut self) {
            LIVE_SURFACE_BYTES.fetch_sub(self.allocation_bytes, Ordering::AcqRel);
        }
    }

    pub fn live_surface_bytes() -> u64 {
        LIVE_SURFACE_BYTES.load(Ordering::Acquire)
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub struct SurfaceToken {
        pub surface_id: u32,
        pub seed: u32,
    }

    pub struct SurfaceReadback {
        pub bgra: Vec<u8>,
        pub stride_bytes: u32,
        pub actual_seed: u32,
    }

    pub fn read_surface(
        surface_id: u32,
        expected_seed: u32,
        width: u32,
        height: u32,
    ) -> Result<SurfaceReadback, SurfaceTransportError> {
        let surface = IOSurfaceRef::lookup(surface_id).ok_or(SurfaceTransportError::NotFound)?;
        if surface.width() != width as usize || surface.height() != height as usize {
            return Err(SurfaceTransportError::InvalidDimensions);
        }
        let mut actual_seed = 0;
        if unsafe { surface.lock(IOSurfaceLockOptions::ReadOnly, &mut actual_seed) } != 0 {
            return Err(SurfaceTransportError::LockFailed);
        }
        if actual_seed < expected_seed {
            let _ = unsafe { surface.unlock(IOSurfaceLockOptions::ReadOnly, &mut actual_seed) };
            return Err(SurfaceTransportError::StaleFrame);
        }
        let stride = surface.bytes_per_row();
        let length = stride.saturating_mul(height as usize);
        let bgra = unsafe {
            slice::from_raw_parts(surface.base_address().as_ptr().cast::<u8>(), length).to_vec()
        };
        if unsafe { surface.unlock(IOSurfaceLockOptions::ReadOnly, &mut actual_seed) } != 0 {
            return Err(SurfaceTransportError::UnlockFailed);
        }
        Ok(SurfaceReadback {
            bgra,
            stride_bytes: stride as u32,
            actual_seed,
        })
    }

    #[derive(Debug, thiserror::Error)]
    pub enum SurfaceTransportError {
        #[error("IOSurface dimensions are invalid")]
        InvalidDimensions,
        #[error("IOSurface could not be created")]
        CreationFailed,
        #[error("IOSurface is no longer available")]
        NotFound,
        #[error("IOSurface pixel buffer is invalid")]
        InvalidBuffer,
        #[error("IOSurface lock failed")]
        LockFailed,
        #[error("IOSurface unlock failed")]
        UnlockFailed,
        #[error("IOSurface contains an older frame")]
        StaleFrame,
    }

    #[cfg(test)]
    mod tests {
        use std::sync::Mutex;

        use super::*;

        static SURFACE_TEST_LOCK: Mutex<()> = Mutex::new(());

        #[test]
        fn iosurface_round_trip_preserves_bgra_pixels() {
            let _guard = SURFACE_TEST_LOCK.lock().unwrap();
            let surface = ProducerSurface::new(2, 2).unwrap();
            let pixels = vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16];
            let token = surface.write_bgra(&pixels, 8).unwrap();
            let readback = read_surface(token.surface_id, token.seed, 2, 2).unwrap();
            assert_eq!(&readback.bgra[..8], &pixels[..8]);
            assert_eq!(readback.actual_seed, token.seed);
        }

        #[test]
        fn iosurface_rejects_stale_seeds_and_mismatched_dimensions() {
            let _guard = SURFACE_TEST_LOCK.lock().unwrap();
            let surface = ProducerSurface::new(2, 2).unwrap();
            let token = surface.write_bgra(&[0; 16], 8).unwrap();
            assert!(matches!(
                read_surface(token.surface_id, token.seed.saturating_add(1), 2, 2),
                Err(SurfaceTransportError::StaleFrame)
            ));
            assert!(matches!(
                read_surface(token.surface_id, token.seed, 1, 2),
                Err(SurfaceTransportError::InvalidDimensions)
            ));
        }

        #[test]
        fn repeated_surface_destruction_invalidates_old_ids() {
            let _guard = SURFACE_TEST_LOCK.lock().unwrap();
            for _ in 0..16 {
                let (surface_id, seed) = {
                    let surface = ProducerSurface::new(1, 1).unwrap();
                    let token = surface.write_bgra(&[1, 2, 3, 4], 4).unwrap();
                    assert!(read_surface(token.surface_id, token.seed, 1, 1).is_ok());
                    (token.surface_id, token.seed)
                };
                assert!(matches!(
                    read_surface(surface_id, seed, 1, 1),
                    Err(SurfaceTransportError::NotFound)
                ));
            }
        }

        #[test]
        fn live_allocation_accounting_covers_zero_one_and_multiple_surfaces() {
            let _guard = SURFACE_TEST_LOCK.lock().unwrap();
            let baseline = live_surface_bytes();
            let first = ProducerSurface::new(64, 64).unwrap();
            assert_eq!(live_surface_bytes(), baseline + first.allocation_bytes());
            let additional = (0..3)
                .map(|_| ProducerSurface::new(64, 64).unwrap())
                .collect::<Vec<_>>();
            let expected = first.allocation_bytes()
                + additional
                    .iter()
                    .map(ProducerSurface::allocation_bytes)
                    .sum::<u64>();
            assert_eq!(live_surface_bytes(), baseline + expected);
            drop(additional);
            drop(first);
            assert_eq!(live_surface_bytes(), baseline);
        }
    }
}

#[cfg(target_os = "macos")]
pub use apple::*;
