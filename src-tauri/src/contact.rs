//! Stride-independent reading of MultitouchSupport contact records.
//!
//! Every known `MTContact` layout starts with six `i32` fields (frame,
//! timestamp, identifier, state, finger id, hand id) and then stores the
//! normalized position as two `f32`s at byte offset 24. The total record size
//! differs between macOS builds (80/88/96 bytes are all published), so we never
//! model the whole struct: the stride is a runtime value and only the position
//! at offset 24 is read.

/// Byte offset of `normalized_x` inside every known contact record.
pub const POSITION_OFFSET: usize = 24;

/// Smallest stride that still contains the position pair.
pub const MIN_STRIDE: usize = POSITION_OFFSET + 8;

/// A normalized trackpad position read out of one contact record.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ContactPos {
    pub x: f32,
    pub y: f32,
}

impl ContactPos {
    pub fn plausible(&self) -> bool {
        coords_plausible(self.x, self.y)
    }
}

/// Coordinates that could be a normalized trackpad position. Slightly outside
/// 0..1 is accepted for firmware jitter; NaN/inf and wild values are not.
pub fn coords_plausible(x: f32, y: f32) -> bool {
    fn one(value: f32) -> bool {
        value.is_finite() && (-0.25..=1.25).contains(&value)
    }
    one(x) && one(y)
}

/// Read `count` positions from `base`, one every `stride` bytes.
///
/// Returns empty for a null base, a non-positive count, or a stride too small
/// to hold the position pair (an unknown layout degrades to "no positions"
/// instead of garbage).
///
/// # Safety contract
/// The caller must guarantee that `count * stride` bytes starting at `base` are
/// readable for the lifetime of this call. The function itself stays safe so it
/// can be unit-tested with plain buffers.
pub fn parse(base: *const u8, count: i32, stride: usize) -> Vec<ContactPos> {
    if base.is_null() || count <= 0 || stride < MIN_STRIDE {
        return Vec::new();
    }

    let mut positions = Vec::with_capacity(count as usize);
    for index in 0..count as usize {
        let record = unsafe { base.add(index * stride) };
        // SAFETY: within the caller-guaranteed `count * stride` bytes, and the
        // stride check above proves the 8 bytes at POSITION_OFFSET exist.
        let x = unsafe { std::ptr::read_unaligned(record.add(POSITION_OFFSET) as *const f32) };
        let y = unsafe { std::ptr::read_unaligned(record.add(POSITION_OFFSET + 4) as *const f32) };
        positions.push(ContactPos { x, y });
    }
    positions
}

#[cfg(test)]
mod tests {
    use super::*;

    fn buffer(stride: usize, count: usize) -> Vec<u8> {
        let mut bytes = vec![0u8; stride * count + 8];
        for index in 0..count {
            let x = 0.25 + index as f32 * 0.1;
            let y = 0.75 - index as f32 * 0.05;
            let at = index * stride + POSITION_OFFSET;
            bytes[at..at + 4].copy_from_slice(&x.to_le_bytes());
            bytes[at + 4..at + 8].copy_from_slice(&y.to_le_bytes());
        }
        bytes
    }

    #[test]
    fn all_known_strides_read_the_same_positions() {
        let expected = [
            ContactPos { x: 0.25, y: 0.75 },
            ContactPos { x: 0.35, y: 0.70 },
            ContactPos { x: 0.45, y: 0.65 },
        ];

        for stride in [80usize, 88, 96] {
            let bytes = buffer(stride, expected.len());
            let parsed = parse(bytes.as_ptr(), expected.len() as i32, stride);
            assert_eq!(parsed, expected, "stride {stride}");
        }
    }

    #[test]
    fn positions_are_read_from_offset_24_only() {
        let stride = 88;
        let mut bytes = vec![0u8; stride];
        let x = 0.5f32.to_le_bytes();
        let y = 0.25f32.to_le_bytes();
        bytes[24..28].copy_from_slice(&x);
        bytes[28..32].copy_from_slice(&y);

        let parsed = parse(bytes.as_ptr(), 1, stride);
        assert_eq!(parsed, vec![ContactPos { x: 0.5, y: 0.25 }]);
    }

    #[test]
    fn garbage_bytes_are_implausible() {
        let bytes = vec![0xffu8; 96 * 2];
        let parsed = parse(bytes.as_ptr(), 2, 88);
        assert_eq!(parsed.len(), 2);
        assert!(parsed.iter().all(|position| !position.plausible()));
    }

    #[test]
    fn plausible_rejects_out_of_range_and_non_finite() {
        assert!(ContactPos { x: 0.0, y: 1.0 }.plausible());
        assert!(ContactPos { x: -0.25, y: 1.25 }.plausible());
        assert!(!ContactPos { x: -0.26, y: 0.5 }.plausible());
        assert!(!ContactPos { x: 0.5, y: 1.26 }.plausible());
        assert!(!ContactPos {
            x: f32::NAN,
            y: 0.5
        }
        .plausible());
        assert!(!ContactPos {
            x: 0.5,
            y: f32::INFINITY
        }
        .plausible());
    }

    #[test]
    fn zero_or_negative_count_is_empty() {
        let bytes = buffer(88, 2);
        assert!(parse(bytes.as_ptr(), 0, 88).is_empty());
        assert!(parse(bytes.as_ptr(), -3, 88).is_empty());
    }

    #[test]
    fn null_base_or_impossible_stride_is_empty() {
        assert!(parse(std::ptr::null(), 3, 88).is_empty());
        assert!(parse(0x1 as *const u8, 3, 0).is_empty());
        assert!(parse(0x1 as *const u8, 3, POSITION_OFFSET).is_empty());
    }
}
