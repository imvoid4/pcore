// exact (x * y) / 255, rounded. Used for all alpha maths.
#[inline(always)]
pub fn mul255(x: u32, y: u32) -> u32 {
    let t = x * y + 128;
    ((t >> 8) + t) >> 8
}
