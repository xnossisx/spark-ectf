use alloc::format;
use alloc::string::ToString;
use core::any::Any;
use crate::pac::Flc;
use core::ptr::null_mut;
use core::result::Result::Err;
use hal::flc::FlashError;
use hal::gcr::clocks::SystemClockResults;

static mut FLASH_HANDLE: *mut hal::flc::Flc = null_mut();

pub fn flash() -> &'static mut hal::flc::Flc {
    unsafe { &mut (*FLASH_HANDLE) }
}

pub fn init(p: Flc, clks: SystemClockResults) {
    unsafe {
        FLASH_HANDLE = &mut hal::flc::Flc::new(p, clks.sys_clk);
    }
}

pub fn read_bytes(frm: u32, dst: &mut [u8], len: usize) -> Result<(), &[u8]> {
    if dst.len() < len {
        return Err(b"FlashError::LowSpace");
    }
    unsafe {
        for i in 0..len / 16 {
            // For 128-bit addresses
            if (dst.as_ptr() as i32) & 0b11 != 0 {
                return Err(b"FlashError::InvalidAddress");
            }
            let addr_128_ptr = ((frm as usize) + i * 16) as u32;
            // Safety: We have checked the address already
            unsafe {
                // Test that unwrap_or_else works correctly
                let res = FLASH_HANDLE.as_ref().unwrap().read_128(addr_128_ptr);
                if res.is_err() {
                    return Err(b"FlashError::ReadFailed");
                }
                *((dst.as_ptr() as usize + i * 16) as *mut [u32; 4]) = res.unwrap();
            }
        }
    }

    Ok(())
}

pub fn write_bytes(dst: u32, from: &[u8], len: usize) -> Result<(), &[u8]> {
    if from.len() < len {
        return Err(b"FlashError::LowSpace");
    }
    unsafe {
        for i in 0..len / 4 {
            // For 128-bit addresses
            if (from.as_ptr() as i32) & 0b11 != 0 {
                return Err(b"FlashError::InvalidAddress");
            }
            let addr_32_ptr = ((dst as usize) + i * 4) as u32;
            // Safety: We have checked the address already
            unsafe {
                // Test that unwrap_or_else works correctly
                let res = (*FLASH_HANDLE).write_32(addr_32_ptr, (from[i * 4] as u32) << (24 + (from[i * 4 + 1] as u32)) << 16 + (from[i * 4 + 2] as u32) << 8 +
                    (from[i * 4 + 3] as u32));
                if res.is_err() {
                    return Err(format!(addr_32_ptr).as_bytes());
                }
            }
        }
    }

    Ok(())
}

pub fn map_err(err: FlashError) -> &'static str {
    match err {
        FlashError::InvalidAddress => "FlashError::InvalidAddress",
        FlashError::AccessViolation => "FlashError::AccessViolation",
        FlashError::NeedsErase => "FlashError::NeedsErase"
    }
}
