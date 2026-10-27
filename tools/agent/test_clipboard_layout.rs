extern crate clipboard;
use clipboard::platform::windows::CliprdrClientContext;

fn main() {
    let expected: usize = std::env::args().nth(1).unwrap().parse().unwrap();
    let expected_offset: usize = std::env::args().nth(2).unwrap().parse().unwrap();
    let storage = std::mem::MaybeUninit::<CliprdrClientContext>::uninit();
    let base = storage.as_ptr();
    let offset = unsafe { std::ptr::addr_of!((*base).ClipboardAccessToken) as usize - base as usize };
    assert_eq!(std::mem::size_of::<CliprdrClientContext>(), expected);
    assert_eq!(offset, expected_offset);
    println!("PASS: compiled Rust/C clipboard ABI size={} callbackOffset={}", expected, offset);
}
