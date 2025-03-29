use core::mem::MaybeUninit;
use core::result::Result::Err;
use hal::flc::{FlashError, Flc};
use hal::gcr::clocks::SystemClockResults;
use hal::pac;

// Core reference to our flash (initially uninitialized)
const FLASH_HANDLE: MaybeUninit<hal::flc::Flc> = MaybeUninit::uninit();

/**
 * Gets a reference to the flash controller
 * @output: An immutable flash controller reference
 */
pub fn flash() -> &'static hal::flc::Flc {
    unsafe { FLASH_HANDLE.assume_init_ref() }
}


/// Creates the flash controller
/// @param p A flash controller
/// @param clks The system clock data
pub fn init(flc: pac::Flc, clks: SystemClockResults) -> Flc {
    Flc::new(flc, clks.sys_clk)
}
/// @param frm The address of the bytes to be read
/// @param dst The reference to the data's destination
/// @param len The size of the bytes to be read
/// @return An error message or nothing on success
pub fn read_bytes<'a>(flc: &Flc, frm: u32, dst: &mut [u8], len: usize) -> Result<(), &'a[u8]> {
    // Checks that the slice has enough space
    if dst.len() < len {
        return Err(b"FlashError::LowSpace");
    }
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
            let res = flc.read_128(addr_128_ptr);
            if res.is_err() {
                return Err(b"FlashError::ReadFailed");
            }
            // Assigns the result to the correct value
            *((dst.as_ptr() as usize + i * 16) as *mut [u32; 4]) = res.unwrap();
        }
        }

    Ok(())
}

/// Writes bytes to the flash
/// @param dst A u32 representing the start address of the write location in flash memory
/// @param from The slice of bytes being written
/// @param len The length of the bytes that will be written
/// @return Either nothing, or the error message
pub fn write_bytes<'a>(flc: &Flc, dst: u32, from: &[u8], len: usize) -> Result<(), &'a [u8]> {
    if from.len() < len {
        return Err(b"FlashError::LowSpace");
    }

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
            let res = flc.write_128(addr_128_ptr, &bytes);
            // Checks for errors
            if res.is_err() {
                return Err(map_err(res.unwrap_err()).as_bytes());
            }
        }
    }
    Ok(())
}

/// Converts flash errors into string messages for write_bytes
/// @param err A flash error
/// @return The corresponding error message
pub fn map_err(err: FlashError) -> &'static str {
    match err {
        FlashError::InvalidAddress => "FlashError::InvalidAddress",
        FlashError::AccessViolation => "FlashError::AccessViolation",
        FlashError::NeedsErase => "FlashError::NeedsErase"
    }
}
