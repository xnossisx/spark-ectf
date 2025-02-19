#![no_std]
#![no_main]

use core::alloc::{GlobalAlloc, Layout};

pub struct Allocator;

unsafe impl GlobalAlloc for Allocator {
    unsafe fn alloc(&self, _layout: Layout) -> *mut u8 {
        0 as *mut u8
    }
    unsafe fn dealloc(&self, _ptr: *mut u8, _layout: Layout) {
        unreachable!(); // since we never allocate
    }
}

#[global_allocator]
static GLOBAL_ALLOCATOR: Allocator = Allocator;

mod console;
mod flash;
mod subscription;
//mod uart;

extern crate alloc;
pub extern crate max7800x_hal as hal;

const NUM_IND: i32 = 16;
pub use hal::entry;
pub use hal::pac;
// pick a panicking behavior
use panic_halt as _;
use flash::flash;
// you can put a breakpoint on `rust_begin_unwind` to catch panics
// use panic_itm as _; // logs messages over ITM; requires ITM support
// use panic_semihosting as _; // logs messages to the host stderr; requires a debugger
// use cortex_m_semihosting::heprintln; // uncomment to use this for printing through semihosting

pub const SUB_LOC: *const u8 = 0x20008000 as *const u8;

#[entry]
fn main() -> ! {
    // heprintln!("Hello from semihosting!");
    let p = pac::Peripherals::take().unwrap();
    //let core = pac::CorePeripherals::take().unwrap();

    let mut gcr = hal::gcr::Gcr::new(p.gcr, p.lpgcr);
    let ipo = hal::gcr::clocks::Ipo::new(gcr.osc_guards.ipo).enable(&mut gcr.reg);
    let clks = gcr
        .sys_clk
        .set_source(&mut gcr.reg, &ipo)
        .set_divider::<hal::gcr::clocks::Div1>(&mut gcr.reg)
        .freeze();

    // Initialize a delay timer using the ARM SYST (SysTick) peripheral
    //let rate = clks.sys_clk.frequency;
    //let mut delay = cortex_m::delay::Delay::new(core.SYST, rate);

    // Initialize and split the GPIO0 peripheral into pins
    let gpio0_pins = hal::gpio::Gpio0::new(p.gpio0, &mut gcr.reg).split();
    // Configure UART to host computer with 115200 8N1 settings
    let rx_pin = gpio0_pins.p0_0.into_af1();
    let tx_pin = gpio0_pins.p0_1.into_af1();
    console::init(p.uart0, &mut gcr.reg, rx_pin, tx_pin, &clks.pclk);

    console::write_console(b"Hello, world!\r\n");

    // Initialize the trng peripheral
    let trng = hal::trng::Trng::new(p.trng, &mut gcr.reg);

    // Load subscription from flash memory
    flash::init(p.flc, clks);

    // Fundamental loop
    loop {}
}

unsafe fn load_subscription(dst: &'static mut [u8]) -> Result<(), &'static [u8]> {
    if dst.len() < 276 {
        console::write_console(b"Insufficient space for subscription");
        return Err(b"");
    }
    if flash().check_address(SUB_LOC as u32).is_err() {
        console::write_console(b"Erroneous flash address");
        return Err(b"");
    }

    panic!();
    Ok(())
}

fn get_id() -> u32 {
    0 //TODO: fix
}
