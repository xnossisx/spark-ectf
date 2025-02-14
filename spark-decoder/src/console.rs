use core::ptr::null_mut;
use hal::gcr::clocks::{Clock, PeripheralClock};
use hal::gcr::GcrRegisters;
use hal::gpio::{Af1, Pin};
use hal::uart::BuiltUartPeripheral;
use crate::pac::Uart0;

static mut CONSOLE_HOOK: *mut BuiltUartPeripheral<Uart0, Pin<0, 0, Af1>, Pin<0, 1, Af1>, (), ()> = null_mut();


pub fn init(uart0:Uart0, reg: &mut GcrRegisters, rx_pin: Pin<0, 0, Af1>, tx_pin: Pin<0, 1, Af1>, pclk: &Clock<PeripheralClock>) {
    unsafe {
        CONSOLE_HOOK = &mut hal::uart::UartPeripheral::uart0(
            uart0,
            reg,
            rx_pin,
            tx_pin
        )
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
