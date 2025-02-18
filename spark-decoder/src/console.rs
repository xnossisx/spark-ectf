use alloc::alloc::alloc;
use core::alloc::Layout;
use crate::pac::Uart0;
use core::ptr::null_mut;
use hal::gcr::clocks::{Clock, PeripheralClock};
use hal::gcr::GcrRegisters;
use hal::gpio::{Af1, Pin};
use hal::uart::BuiltUartPeripheral;

static mut CONSOLE_HOOK: *mut BuiltUartPeripheral<Uart0, Pin<0, 0, Af1>, Pin<0, 1, Af1>, (), ()> =
    null_mut();

pub fn init(
    uart0: Uart0,
    reg: &mut GcrRegisters,
    rx_pin: Pin<0, 0, Af1>,
    tx_pin: Pin<0, 1, Af1>,
    pclk: &Clock<PeripheralClock>,
) {
    unsafe {
        CONSOLE_HOOK = &mut hal::uart::UartPeripheral::uart0(uart0, reg, rx_pin, tx_pin)
            .baud(115200)
            .clock_pclk(&pclk)
            .parity(hal::uart::ParityBit::None)
            .build();
    }
}

pub fn write_console(bytes: &[u8]) {
    unsafe {
        (*CONSOLE_HOOK).write_bytes(bytes);
    }
}

pub fn read_byte() -> u8 {
    unsafe {(*CONSOLE_HOOK).read_byte()}
}

pub fn ack() {
    write_console(b"\x07");
}

pub fn read_resp() {
    let magic = read_byte();
    let opcode = read_byte();
    let length: u16 = ((read_byte() as u16) << 8) + (read_byte() as u16);
    match opcode {
        b'S' => {
            
        },
        b'L' => {

        },
        b'D' => {
            unsafe {
                let byte_list = alloc(Layout::from_size_align(length as usize, 16).unwrap());
            }        
        },
        b'A' => {

        },
        b'E' => {

        },
        b'G' => {
            
        },
        _ => write_console(b"weird opcode")
    };
}