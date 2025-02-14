#![no_std]
#![no_main]

use core::alloc::{GlobalAlloc, Layout};

pub struct Allocator;

unsafe impl GlobalAlloc for Allocator {
    unsafe fn alloc(&self, _layout: Layout) -> *mut u8 {
        0 as *mut u8
    }
    unsafe fn dealloc(&self, _ptr: *mut u8, _layout: Layout) {
         unreachable!();     // since we never allocate
    }
}

#[global_allocator]
static GLOBAL_ALLOCATOR: Allocator = Allocator;

mod flash;
mod subscription;
mod console;

pub extern crate max7800x_hal as hal;
use core::ffi::c_void;

use rug::Integer;
extern crate alloc;

const NUM_IND: i32 = 16;
use core::ops::Deref;
pub use hal::pac;
pub use hal::entry;
use hal::pac::{gcr, Flc, Peripherals};
use hal::uart::{BuiltUartPeripheral, UartPeripheral};
// pick a panicking behavior
use flash::flash;
// use panic_halt as panic;
use crate::console::write_err;
use crate::pac::Uart0;
// you can put a breakpoint on `rust_begin_unwind` to catch panics
// use panic_itm as _; // logs messages over ITM; requires ITM support
// use panic_semihosting as _; // logs messages to the host stderr; requires a debugger
// use cortex_m_semihosting::heprintln; // uncomment to use this for printing through semihosting

const SUB_LOC: *const u8 = 0x20008000 as *const u8;



#[entry]
fn main() -> ! {
    // heprintln!("Hello from semihosting!");
    let p = pac::Peripherals::take().unwrap();
    let core = pac::CorePeripherals::take().unwrap();

    let mut gcr = hal::gcr::Gcr::new(p.gcr, p.lpgcr);
    let ipo = hal::gcr::clocks::Ipo::new(gcr.osc_guards.ipo).enable(&mut gcr.reg);
    let clks = gcr.sys_clk
        .set_source(&mut gcr.reg, &ipo)
        .set_divider::<hal::gcr::clocks::Div1>(&mut gcr.reg)
        .freeze();

    // Initialize a delay timer using the ARM SYST (SysTick) peripheral
    let rate = clks.sys_clk.frequency;
    let mut delay = cortex_m::delay::Delay::new(core.SYST, rate);

    // Initialize and split the GPIO0 peripheral into pins
    let gpio0_pins = hal::gpio::Gpio0::new(p.gpio0, &mut gcr.reg).split();
    // Configure UART to host computer with 115200 8N1 settings
    let rx_pin = gpio0_pins.p0_0.into_af1();
    let tx_pin = gpio0_pins.p0_1.into_af1();
    console::init(p.uart0, &mut gcr.reg, rx_pin,tx_pin, &clks.pclk);

    console::write_console(b"Hello, world!\r\n");

    {
        // Initialize the GPIO2 peripheral
        let pins = hal::gpio::Gpio2::new(p.gpio2, &mut gcr.reg).split();
        // Enable output mode for the RGB LED pins
        let mut led_r = pins.p2_0.into_input_output();
        let mut led_g = pins.p2_1.into_input_output();
        let mut led_b = pins.p2_2.into_input_output();
        // Use VDDIOH as the power source for the RGB LED pins (3.0V)
        // Note: This HAL API may change in the future
        led_r.set_power_vddioh();
        led_g.set_power_vddioh();
        led_b.set_power_vddioh();

        led_r.set_high();
        delay.delay_ms(500);
        led_g.set_high();
        delay.delay_ms(500);
        led_b.set_high();
        delay.delay_ms(500);
        led_r.set_low();
        delay.delay_ms(500);
        led_g.set_low();
        delay.delay_ms(500);
        led_b.set_low();
        delay.delay_ms(500);
    }

    // Initialize the trng peripheral
    let trng = hal::trng::Trng::new(p.trng, &mut gcr.reg);

    // Load subscription from flash memory
    flash::init(p.flc, clks);


    // Fundamental loop
    loop {

    }
}

unsafe fn load_subscription(dst: &'static mut [u8]) -> Result<(), &'static [u8]> {
    if dst.len() < 276 {
        return console::write_err(b"Insufficient space for subscription\n")
    }
    if flash().check_address(SUB_LOC as u32).is_err() {
        return console::write_err(b"Erroneous flash address")
    }

    panic!();
    Ok(())
}

fn get_id() -> u32 {
    0 //TODO: fix
}