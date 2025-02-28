#![feature(concat_bytes)]
#![no_std]
#![no_main]

use cortex_m::asm::delay;
use alloc::fmt::format;
use alloc::format;
use alloc::string::ToString;
use hal::trng::Trng;
use core::alloc::GlobalAlloc;
use core::cell::RefCell;
use core::panic::PanicInfo;
use cortex_m::delay::Delay;
use crypto_bigint::{Encoding, Odd, Zero, U1024};
use embedded_alloc::LlffHeap;
use hmac_sha512;

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
const SUB_SIZE: u32 = 8192; /*two keys + key lengths + modulus + channel + start + end*/
pub use hal::entry;
use hal::flc::FlashError;
pub use hal::pac;
use flash::flash;
use crate::console::cons;
use crate::subscription::Subscription;
// you can put a breakpoint on `rust_begin_unwind` to catch panics
// use panic_itm as _; // logs messages over ITM; requires ITM support
// use panic_semihosting as _; // logs messages to the host stderr; requires a debugger
// use cortex_m_semihosting::heprintln; // uncomment to use this for printing through semihosting

/**
 * The location of all of our subscription data on the flash
*/
pub const SUB_LOC: *const u8 = 0x1001f000 as *const u8;

#[entry]
fn main() -> ! {
    // Initializes our heap
    {
        use core::mem::MaybeUninit;
        const HEAP_SIZE: usize = 1024;
        static mut HEAP_MEM: [MaybeUninit<u8>; HEAP_SIZE] = [MaybeUninit::uninit(); HEAP_SIZE];
        unsafe { HEAP.init(HEAP_MEM.as_ptr() as usize, HEAP_SIZE) }
    }

    // Initialize peripherals
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

    let pins = hal::gpio::Gpio2::new(p.gpio2, &mut gcr.reg).split();

    let mut led_r = pins.p2_0.into_input_output();
    let mut led_g = pins.p2_1.into_input_output();
    let mut led_b = pins.p2_2.into_input_output();

    // Initialize the trng peripheral
    let trng = Trng::new(p.trng, &mut gcr.reg);


    // Use VDDIOH as the power source for the RGB LED pins (3.0V)
    // Note: This HAL API may change in the future

    // Initialize a delay timer using the ARM SYST (SysTick) peripheral
    let rate = clks.sys_clk.frequency;

    let mut delay = cortex_m::delay::Delay::new(core.SYST, rate);

    // Load subscription from flash memory
    flash::init(p.flc, clks);
    let mut subscriptions: [Subscription; 8] = load_subscriptions(console);



    // Fundamental event loop
    loop {
        //
        console::write_console(console,b"!\n");
        // Delays to avoid side channel attacks
        let test_val = trng.gen_u32();

        //
        let output = test(test_val, &trng, &mut delay);
        if test_val*test_val == output {
            console::read_resp(&mut subscriptions, console);
        }
    }
}

/**
 * Likely to be exposed to data corruption, thereby allowing us to detect interference
 */
fn test(scan: u32, trng: &Trng, delay: &mut Delay) -> u32 {
    let ret = scan*scan;
    delay.delay_us(5u32 + (trng.gen_u32() & 511));
    return ret
}

/**
 * Reads all subscriptions from the flash
 * Acts as a wrapper to load_subscription
 */
fn load_subscriptions(console: &console::cons) -> [Subscription; 8] {

        // Page 1: Modulus, Channel, Start, End, Forward Count, Backward Count
        // Page 2: Forward exponents, Backward exponents
        let mut ret = [Subscription::new(); 8];

        //let layout = Layout::from_size_align((SUB_SIZE * 8) as usize, 8).unwrap();
        //let mut forward_backward: *mut u8 = alloc(layout);
        for i in 0usize..8 {
            if !load_subscription(&mut ret[i], console, i) {
                break;
            }
        }
        ret
}


/**
 * Reads a subscription from the flash
 * Acts as a wrapper to load_subscription
 * @param channel_pos: A value from 0 to 8
 * @param cons: A reference to the console object
 * @param subscription: A reference to a subscription being loaded
 * @return The success of the operation
 */
fn load_subscription(subscription: &mut Subscription, console: &cons, channel_pos: usize) -> bool {
    let cache: RefCell<[u8; 2048 as usize]> = RefCell::new([0; 2048 as usize]);
    let mut pos: usize = (channel_pos * SUB_SIZE as usize);
    console::write_console(console, format!("{:x}", (SUB_LOC as u32) + pos as u32).as_bytes());
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
        };
        return false
    }
    unsafe {
        console::write_console(console, b"started");
        let _ = flash::read_bytes(SUB_LOC as u32 + pos as u32, &mut (*cache.as_ptr()), 2048 as usize);
        console::write_console(console, b"read");

        let init = (*cache.as_ptr())[4]; // Should always be non-zero if it's loaded right
        if init == 0 || init == 0xFF {
            return false;
        } else {
            console::write_console(console, &[init]);
        }

        subscription.location = pos;
        subscription.channel=u32::from_be_bytes((*cache.as_ptr())[pos..pos+4].try_into().unwrap());
        pos += 4;

        pos += 2;

        for j in 0..16 {
            let val = u64::from_be_bytes((*cache.as_ptr())[pos + j*8 ..pos + j*8 + 8].try_into().unwrap());
            if (val == 0 && j > 0) {
                break;
            }
            subscription.forward_pos[j] = val;
        }
        pos += 128;

        for j in 0..16 {
            let val = u64::from_be_bytes((*cache.as_ptr())[pos + j*8 ..pos + j*8 + 8].try_into().unwrap());
            if (val == 0 && j > 0) {
                break;
            }
            subscription.backward_pos[j] = val;
        }
        pos += 128;
        console::write_console(console, format!("{:#x}", subscription.channel).as_bytes());

        console::write_console(console, b"hook");
        subscription.n=Odd::<Integer>::new(Integer::from_be_bytes((*cache.as_ptr())[pos ..pos + 128].try_into().unwrap())).unwrap();
        pos += 128;

        subscription.start=u64::from_be_bytes((*cache.as_ptr())[pos..pos+8].try_into().unwrap());
        pos += 8;
        subscription.end=u64::from_be_bytes((*cache.as_ptr())[pos..pos+8].try_into().unwrap());
    }

    true
}

/**
 * @output The ID of the decoder, as indicated by compilation.
 */
fn get_id() -> u32 {
    env!("DECODER_ID").parse::<u32>().unwrap()
}

///Outputs the SHA-3 hash of the device ID
static mut HASH: Integer = Integer::ZERO;
fn get_hashed_id() -> &'static Integer {
    unsafe {
        if HASH.is_zero().into() {
            let output: [u8;128]=[0; 128];
            output.copy_from_slice(&hmac_sha512::Hash::hash(get_id().to_be_bytes().as_ref()));
            HASH = Integer::from_be_bytes(output);
        }
        &HASH
    }
}

fn get_channels() -> [u32; 9] {
    let mut ret: [u32; 9] = [0; 9];
    // Unfortunately, Rust does not let you put format strings into environment variables.
    ret[0] = 0;
    // Get the channels from the environment variable CHANNELS, which is like "1,3,7,8" or something
    let channels = env!("CHANNELS");
    let mut i = 1;
    for channel in channels.split(",") {
        ret[i] = channel.parse::<u32>().unwrap();
        i += 1;
    }

    ret
}

fn get_loc_for_channel(channel: u32) -> u32 {
    let channels = get_channels();
    for i in 0..channels.len() {
        if channels[i] == channel {
            return i as u32;
        }
    }
    0
}


/**
 * Allows for simple panicking.
 */
#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}
