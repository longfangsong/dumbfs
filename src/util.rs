use num::Integer;

pub fn align<T: Integer + Copy>(num: T, to: T) -> T {
    if to == T::zero() || num % to == T::zero() {
        num
    } else {
        num / to * to + to
    }
}

#[test]
fn test_align() {
    assert_eq!(align(0, 512), 0);
    assert_eq!(align(127, 0), 127);
    assert_eq!(align(0u64, 512u64), 0);
    assert_eq!(align(0usize, 512usize), 0);
    assert_eq!(align(512, 512), 512);
    assert_eq!(align(128, 512), 512);
    assert_eq!(align(513, 512), 1024);
}
