use alloc::format;
use alloc::string::ToString;
use core::any::Any;
use core::mem::MaybeUninit;
use crate::pac::Flc;
use core::ptr::null_mut;
use core::result::Result::Err;
use hal::flc::FlashError;
use hal::gcr::clocks::SystemClockResults;

// Core reference to our flash (initially uninitialized)
const FLASH_HANDLE: MaybeUninit<*mut hal::flc::Flc> = MaybeUninit::uninit();

/**
 * Gets a reference to the flash controller
 * @output: An immutable flash controller reference
 */
pub fn flash() -> &'static hal::flc::Flc {
    unsafe { & *FLASH_HANDLE.as_mut_ptr() }
}

/**
 * Gets a reference to the flash controller
 * @param p: A flash controller
 * @param clks: The system clock data
 */
pub fn init(p: Flc, clks: SystemClockResults) {
    unsafe {
        FLASH_HANDLE = &mut hal::flc::Flc::new(p, clks.sys_clk);
    }
}

///
/// Reads bytes from the flash memory
/// @param p: A flash controller
/// @param clks: The system clock data
/// @param len: The size of the bytes to be read
/// @output: An error message or nothing
///
pub fn read_bytes(frm: u32, dst: &mut [u8], len: usize) -> Result<(), &[u8]> {
    // Checks that the slice has enough space
    if dst.len() < len {
        return Err(b"FlashError::LowSpace");
    }
    unsafe {
        // Reads values 128 bits at a time
        for i in 0..len / 16 {
            // Verifies the address
            if (dst.as_ptr() as i32) & 0b11 != 0 {
                return Err(b"FlashError::InvalidAddress");
            }
            let addr_128_ptr = ((frm as usize) + i * 16) as u32;
            // Security guarantee: We have checked the address already
            unsafe {
                // Collects the result and checks it for errors
                let res = flash().read_128(addr_128_ptr);
                if res.is_err() {
                    return Err(b"FlashError::ReadFailed");
                }
                // Assigns the result to the correct value
                *((dst.as_ptr() as usize + i * 16) as *mut [u32; 4]) = res.unwrap();
            }
        }
    }

    Ok(())
}

/// Writes bytes to the flash
/// dst: A u32 representing the start address of the write location in flash memory
/// from: The slice of bytes being written
/// len: The length of the bytes that will be written
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
            // We have checked the address already
            unsafe {
                // Performs write
                let res = flash().write_32(addr_32_ptr, (from[i * 4] as u32) << (24 + (from[i * 4 + 1] as u32)) << 16 + (from[i * 4 + 2] as u32) << 8 +
                    (from[i * 4 + 3] as u32));
                // Checks for errors
                if res.is_err() {
                    return Err(format!(addr_32_ptr).as_bytes());
                }
            }
        }
    }

    Ok(())
}

/// Converts flash errors into string messages
pub fn map_err(err: FlashError) -> &'static str {
    match err {
        FlashError::InvalidAddress => "FlashError::InvalidAddress",
        FlashError::AccessViolation => "FlashError::AccessViolation",
        FlashError::NeedsErase => "FlashError::NeedsErase"
    }
}
