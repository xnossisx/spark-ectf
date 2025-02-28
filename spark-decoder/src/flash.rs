use core::mem::MaybeUninit;
use core::result::Result::Err;
use hal::flc::FlashError;
use hal::gcr::clocks::SystemClockResults;
use hal::pac::Flc;
use crate::console::cons;


static mut FLASH_HANDLE: &hal::flc::Flc = &MaybeUninit::zeroed();
// Core reference to our flash (initially uninitialized)
const FLASH_HANDLE: MaybeUninit<hal::flc::Flc> = MaybeUninit::uninit();

/**
 * Gets a reference to the flash controller
 * @output: An immutable flash controller reference
 */
pub fn flash() -> &'static hal::flc::Flc {
    unsafe { FLASH_HANDLE.assume_init_ref() }
}

/**
 * Gets a reference to the flash controller
 * @param p: A flash controller
 * @param clks: The system clock data
 */
pub fn init(p: Flc, clks: SystemClockResults) {
    unsafe {
        FLASH_HANDLE.write(hal::flc::Flc::new(p, clks.sys_clk));
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
pub fn write_bytes<'a>(dst: u32, from: &[u8], len: usize, console: &cons) -> Result<(), &'a [u8]> {
    if from.len() < len {
        return Err(b"FlashError::LowSpace");
    }
    unsafe {
        for i in 0usize..len / 16 {
            // For 128-bit addresses
            if (from.as_ptr() as i32) & 0b11 != 0 {
                return Err(b"FlashError::InvalidAddress");
            }
            let addr_128_ptr = ((dst as usize) + i * 16) as u32;
            // We have checked the address already
            unsafe {
                // Performs write
                let bytes: [u32; 4]  = *((from.as_ptr() as usize + i * 16) as *const [u32; 4]);
                let res = FLASH_HANDLE.assume_init_ref().write_128(addr_128_ptr, &bytes);
                // Checks for errors
                if res.is_err() {
                    return Err(map_err(res.unwrap_err()).as_bytes());
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
