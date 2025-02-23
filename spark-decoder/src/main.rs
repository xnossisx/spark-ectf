#![feature(concat_bytes)]
#![no_std]
#![no_main]

use alloc::boxed::Box;
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
mod commands;
//mod uart;

extern crate alloc;
pub extern crate max7800x_hal as hal;

const NUM_IND: i32 = 16;
const SUB_SIZE: u32 = 8192*3; /*two keys + key lengths + modulus + channel + start + end*/
pub use hal::entry;
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

pub const SUB_LOC: *const u8 = 0x20008000 as *const u8;

#[entry]
fn main() -> ! {

    {
        use core::mem::MaybeUninit;
        const HEAP_SIZE: usize = 1024;
        static mut HEAP_MEM: [MaybeUninit<u8>; HEAP_SIZE] = [MaybeUninit::uninit(); HEAP_SIZE];
        unsafe { HEAP.init(HEAP_MEM.as_ptr() as usize, HEAP_SIZE) }
    }

    // heprintln!("Hello from semihosting!");
    let p = pac::Peripherals::take().unwrap();
    let core = pac::CorePeripherals::take().unwrap();

    let mut gcr = hal::gcr::Gcr::new(p.gcr, p.lpgcr);
    let ipo = hal::gcr::clocks::Ipo::new(gcr.osc_guards.ipo).enable(&mut gcr.reg);
    let clks = gcr
        .sys_clk
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
    console::init(p.uart0, &mut gcr.reg, rx_pin, tx_pin, &clks.pclk);

    console::write_console(b"Hello, world!\r\n");

    // Initialize the trng peripheral
    let trng = hal::trng::Trng::new(p.trng, &mut gcr.reg);

    // Load subscription from flash memory
    flash::init(p.flc, clks);
    let mut subscriptions: [Subscription; 8] = (*load_subscriptions().as_slice()).try_into().unwrap();


    // Fundamental loop
    loop {
        delay.delay_us(5u32 + (trng.gen_u32() & 511));
        let val = console::read_resp();
        if val.is_some() {
            process(val.unwrap());
        }
    }
}

fn process(p0: &[u8]) {

}

fn load_subscriptions() -> Box<[Subscription; 8]> {
    unsafe {
        if flash().check_address(SUB_LOC as u32).is_err() {
            console::write_err(b"Erroneous flash address");
            //return Err(b"");
        }

        // Page 1: Modulus, Channel, Start, End, Forward Count, Backward Count
        // Page 2: Forward exponents, Backward exponents

        let mut ret = Box::new([Subscription::new(); 8]);
        //let layout = Layout::from_size_align((SUB_SIZE * 8) as usize, 8).unwrap();
        let mut forward_backward: RefCell<[u8; (SUB_SIZE * 8) as usize]> = RefCell::new([0; (SUB_SIZE * 8) as usize]);
        //let mut forward_backward: *mut u8 = alloc(layout);
        for i in 0usize..8 {
            //let mut sub_data = [0u8; 8192];
            let mut for_size = 0;
            let mut back_size = 0;

            let mut pos: usize = (i*SUB_SIZE as usize);

            let _ = flash::read_bytes(SUB_LOC as u32 + pos as u32, &mut (*forward_backward.as_ptr())[0..SUB_SIZE as usize], SUB_SIZE as usize);
            if (*forward_backward.as_ptr())[0] == 0 {
                break;
            }
            // Part 1: the first page
            // The first two bytes are padding
            // The next 512 bytes are the forward key locations
            pos += 2;
            let mut subscription = ret.as_mut_slice()[i as usize];
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
            }
        }
        ret
    }
}

fn get_id() -> u32 {
    env!("DECODER_ID").parse::<u32>().unwrap()
}
