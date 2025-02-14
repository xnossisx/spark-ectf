use core::ptr::null_mut;
use hal::flc::FlashError;
use hal::gcr::clocks::SystemClockResults;
use hal::pac::Peripherals;
use crate::pac::Flc;
use crate::console::write_err;
use core::result::Result::Err;

static mut FLASH_HANDLE: *mut hal::flc::Flc = null_mut();

pub fn flash() -> &'static mut hal::flc::Flc {
    unsafe { &mut (*FLASH_HANDLE) }
}

pub fn init(p: Flc, clks: SystemClockResults) {
    unsafe {
        FLASH_HANDLE = &mut hal::flc::Flc::new(p, clks.sys_clk);
    }
}

fn read_bytes(frm: u32, dst: &mut [u8], len: usize) -> Result<(), &[u8]> {
    if dst.len() < len {
        return Err(b"Insufficient space for destination\n")
    }
    unsafe {
        for i in 0..len / 16 { // For 128-bit addresses
            if (dst.as_ptr() as i32) & 0b11 != 0 {
                return Err(b"FlashError::InvalidAddress");
            }
            let addr_128_ptr = ((frm as usize) +i*16) as u32;
            // Safety: We have checked the address already
            unsafe {
                // Test that unwrap_or_else works correctly
                let res = FLASH_HANDLE.as_ref().unwrap().read_128(addr_128_ptr);
                if res.is_err() {
                    return Err(b"FlashError::ReadFailed");
                }
                *(((dst.as_ptr() as usize + i*16)) as *mut [u32; 4]) = res.unwrap();
            }
        }
    }

    Ok(())
}