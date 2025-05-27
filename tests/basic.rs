use std::path::Path;

use coreclr_hosting_shared::{StatusCode, char_t};

#[test]
fn returned_path_exists() {
    let mut buffer: Vec<char_t> = Vec::new();
    let mut buffer_size = buffer.len();

    let result = unsafe {
        nethost_sys::get_hostfxr_path(
            buffer.as_mut_ptr().cast(),
            &mut buffer_size,
            core::ptr::null(),
        )
    };
    assert_eq!(result, StatusCode::HostApiBufferTooSmall as i32);
    buffer.reserve(buffer_size);

    let result = unsafe {
        nethost_sys::get_hostfxr_path(
            buffer.as_mut_ptr().cast(),
            &mut buffer_size,
            core::ptr::null(),
        )
    };
    assert_eq!(result, StatusCode::Success as i32);
    unsafe { buffer.set_len(buffer_size) };

    buffer.truncate(buffer_size - 1);

    #[cfg(windows)]
    let path = String::from_utf16(&buffer).unwrap();
    #[cfg(not(windows))]
    let path = String::from_utf8(buffer).unwrap();

    assert!(Path::new(&path).exists());
}
