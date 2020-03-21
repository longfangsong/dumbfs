pub fn align(num: u64) -> u64 {
    if num % 512 == 0 {
        num
    } else {
        num / 512 * 512 + 512
    }
}

#[test]
fn test_align() {
    assert_eq!(align(0), 0);
    assert_eq!(align(512), 512);
    assert_eq!(align(128), 512);
    assert_eq!(align(513), 1024);
}
