#![feature(concat_bytes)]
#![no_std]
#![no_main]

use alloc::boxed::Box;
use alloc::string::ToString;
use core::alloc::{GlobalAlloc, Layout};
use core::cell::RefCell;
use crypto_bigint::{Encoding, Int, Odd, U1024};
use embedded_alloc::LlffHeap;

type Integer = U1024;
type Heap = LlffHeap;

#[global_allocator]
static HEAP: Heap = Heap::empty();

mod console;
mod flash;
mod subscription;
//mod uart;

extern crate alloc;
pub extern crate max7800x_hal as hal;
const NUM_IND: i32 = 16;
const SUB_SIZE: u32 = 8192 * 3; /*two keys + key lengths + modulus + channel + start + end*/
pub use hal::entry;
use hal::flc::FlashError;
pub use hal::pac;
use hal::pac::gpio0::In;
// pick a panicking behavior
use panic_halt as _;
use flash::flash;
use crate::subscription::Subscription;
// you can put a breakpoint on `rust_begin_unwind` to catch panics
// use panic_itm as _; // logs messages over ITM; requires ITM support
// use panic_semihosting as _; // logs messages to the host stderr; requires a debugger
// use cortex_m_semihosting::heprintln; // uncomment to use this for printing through semihosting

pub const SUB_LOC: *const u8 = 0x10020000 as *const u8;
#[entry]
fn main() -> ! {
    {
        use core::mem::MaybeUninit;
        const HEAP_SIZE: usize = 1024;
        static mut HEAP_MEM: [MaybeUninit<u8>; HEAP_SIZE] = [MaybeUninit::uninit(); HEAP_SIZE];
        unsafe { HEAP.init(HEAP_MEM.as_ptr() as usize, HEAP_SIZE) }
    }

    let p = pac::Peripherals::take().unwrap();
    let core = pac::CorePeripherals::take().unwrap();
    let mut gcr = hal::gcr::Gcr::new(p.gcr, p.lpgcr);
    let ipo = hal::gcr::clocks::Ipo::new(gcr.osc_guards.ipo).enable(&mut gcr.reg);
    let clks = gcr
        .sys_clk
        .set_source(&mut gcr.reg, &ipo)
        .set_divider::<hal::gcr::clocks::Div1>(&mut gcr.reg)
        .freeze();

    // Initialize and split the GPIO0 peripheral into pins
    let gpio0_pins = hal::gpio::Gpio0::new(p.gpio0, &mut gcr.reg).split();
    // Configure UART to host computer with 115200 8N1 settings
    let rx_pin = gpio0_pins.p0_0.into_af1();
    let tx_pin = gpio0_pins.p0_1.into_af1();
    let console = &console::init(p.uart0, &mut gcr.reg, rx_pin, tx_pin, &clks.pclk);

    // Initialize the trng peripheral
    //let trng = hal::trng::Trng::new(p.trng, &mut gcr.reg);

    let pins = hal::gpio::Gpio2::new(p.gpio2, &mut gcr.reg).split();

    let mut led_r = pins.p2_0.into_input_output();
    let mut led_g = pins.p2_1.into_input_output();
    let mut led_b = pins.p2_2.into_input_output();
    // Use VDDIOH as the power source for the RGB LED pins (3.0V)
    // Note: This HAL API may change in the future

    // Initialize a delay timer using the ARM SYST (SysTick) peripheral
    let rate = clks.sys_clk.frequency;

    let mut delay = cortex_m::delay::Delay::new(core.SYST, rate);



    // Load subscription from flash memory
    flash::init(p.flc, clks);
    let mut subscriptions: [Subscription; 8] = load_subscriptions(console);



    // Fundamental loop
    loop {
        console::write_console(console,b"!\n");

        //delay.delay_us(5u32 + (trng.gen_u32() & 511));
        console::read_resp(&mut subscriptions, console);
    }
}

fn load_subscriptions(console: &console::cons) -> [Subscription; 8] {
    unsafe {


        // Page 1: Modulus, Channel, Start, End, Forward Count, Backward Count
        // Page 2: Forward exponents, Backward exponents
        let mut ret = [Subscription::new(); 8];

        //let layout = Layout::from_size_align((SUB_SIZE * 8) as usize, 8).unwrap();
        let mut forward_backward: RefCell<[u8; 2048 as usize]> = RefCell::new([0; 2048 as usize]);
        //let mut forward_backward: *mut u8 = alloc(layout);
        for i in 0usize..8 {
            //let mut sub_data = [0u8; 8192];
            let mut for_size = 0;
            let mut back_size = 0;

            let mut pos: usize = (i*SUB_SIZE as usize);
            let result = flash().check_address(SUB_LOC as u32 + pos as u32);
            if result.is_err() {
                match result.unwrap_err() {
                    FlashError::InvalidAddress => {
                        console::write_console(console, b"InvalidAddress\n");
                    }
                    FlashError::AccessViolation => {
                        console::write_console(console, b"InvalidOperation\n");
                    }
                    FlashError::NeedsErase => {
                        console::write_console(console, b"NeedsErase\n");
                    }

                }
            }
            let _ = flash::read_bytes(SUB_LOC as u32 + pos as u32, &mut (*forward_backward.as_ptr()), 2048 as usize);
            let init = (*forward_backward.as_ptr())[0];
            if init == 0 || init == 0xFF {
                break;
            }
            // Part 1: the first page
            // The first two bytes are padding
            // The next 512 bytes are the forward key locations

            let mut subscription = ret.as_mut_slice()[i as usize];

            subscription.location = pos;
            pos += 2;
            for j in 0..64 {
                let val = u64::from_be_bytes((*forward_backward.as_ptr())[pos + j*8 ..pos + j*8 + 8].try_into().unwrap());
                if (val == 0 && j > 0) {
                    break;
                }
                subscription.forward_pos[j] = val;
            }

            pos += 512;
            for j in 0..64 {
                let val = u64::from_be_bytes((*forward_backward.as_ptr())[pos + j*8 ..pos + j*8 + 8].try_into().unwrap());
                if (val == 0 && j > 0) {
                    break;
                }
                subscription.backward_pos[j] = val;
            }

            pos += 512;
            subscription.n=Odd::<Integer>::new(Integer::from_be_bytes((*forward_backward.as_ptr())[pos ..pos + 128].try_into().unwrap())).unwrap();

            pos += 128;
            subscription.channel=u32::from_be_bytes((*forward_backward.as_ptr())[pos..pos+4].try_into().unwrap());

            pos += 4;
            subscription.start=u64::from_be_bytes((*forward_backward.as_ptr())[pos..pos+8].try_into().unwrap());

            pos += 8;
            subscription.end=u64::from_be_bytes((*forward_backward.as_ptr())[pos..pos+8].try_into().unwrap());
/*
            pos = (i*SUB_SIZE as usize) + 8192;
            for j in 0..64 {
                let val = Integer::from_be_bytes((*forward_backward.as_ptr())[pos + j*128 ..pos + (j+1)*128].try_into().unwrap());
                if val == Integer::ZERO && j > 0 {
                    break;
                }
                subscription.forward_refs[j] = val;
            }

            pos = (i*SUB_SIZE as usize) + 16384;
            for j in 0..64 {
                let val = Integer::from_be_bytes((*forward_backward.as_ptr())[pos + j*128 ..pos + (j+1)*128].try_into().unwrap());
                if val == Integer::ZERO && j > 0 {
                    break;
                }
                subscription.back_refs[j] = val.bitxor(&Integer::from(get_id()));
            }*/
        }
        ret
    }
}
fn get_id() -> u32 {
    env!("DECODER_ID").parse::<u32>().unwrap()
}