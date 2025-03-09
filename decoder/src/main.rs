#![feature(concat_bytes)]
#![no_std]
#![no_main]

use alloc::format;
use alloc::string::ToString;
use hal::trng::Trng;
use core::alloc::GlobalAlloc;
use core::cell::RefCell;
use core::panic::PanicInfo;
use cortex_m::delay::Delay;
use dashu_int::fast_div::ConstDivisor;
use dashu_int::UBig;
use embedded_alloc::LlffHeap;
use hmac_sha512;


type Heap = LlffHeap;

#[global_allocator]
static HEAP: Heap = Heap::empty();

mod console;
mod flash;
mod subscription;
//mod uart;

extern crate alloc;
pub extern crate max7800x_hal as hal;
const SUB_SPACE: u32 = 8192; /* page length */
const SUB_SIZE: usize = 4+2+64+64+128+8+8+1024+1024;
/* channel # + intermediate lengths + intermediate references + modulus + start + end +
intermediates (1024*2)*/

pub const INTERMEDIATE_NUM: usize = 64;
pub const INTERMEDIATE_LOC: u32 = 1280;
pub const INTERMEDIATE_SIZE: usize = 16;
pub const INTERMEDIATE_POS_SIZE: usize = 8;
pub 

use hal::entry;
use hal::flc::FlashError;
pub use hal::pac;
use flash::flash;
use crate::console::{cons, console, write_console};
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
    let _ = &console::init(p.uart0, &mut gcr.reg, rx_pin, tx_pin, &clks.pclk);

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

    let mut delay = Delay::new(core.SYST, rate);

    // Load subscription from flash memory
    let flash = flash::init(p.flc, clks);
    //let mut subscriptions: [Subscription; 9] = load_subscriptions(&flash);
    let mut moduli: [ConstDivisor; 9] = load_moduli(&flash);


    // Fundamental event loop
    loop {
        // Delays to avoid side channel attacks
        let test_val = trng.gen_u32();

        let output = test(test_val, &trng, &mut delay);
        if test_val*test_val == output {
            console::read_resp(&flash, &mut moduli);
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

fn load_moduli(flash: &hal::flc::Flc) -> [ConstDivisor; 9] {
    let mut ret  = core::array::from_fn(|i| ConstDivisor::new(UBig::from(0)));
    let mut cache: [u8; 160] = [0; 160];

    for i in 0usize..8 {
        let pos = (SUB_SPACE as usize) * i + INTERMEDIATE_POS_SIZE * INTERMEDIATE_NUM + 22usize;
        flash::read_bytes(flash, pos as u32, &mut cache, 160);
        let encrypted_modulus = UBig::from_be_bytes(&cache[0..160]);
        ret[i]=decrypt_channel_modulus(encrypted_modulus, i as u32);
    }
    ret
}

/**
 * Reads all subscriptions from the flash
 * Acts as a wrapper to load_subscription
 */
fn load_subscriptions(flash: &hal::flc::Flc) -> [Subscription; 9] {
    // Page 1: Modulus, Channel, Start, End, Forward Count, Backward Count
    // Page 2: Forward exponents, Backward exponents
    let mut ret  = core::array::from_fn(|i| Subscription::new());

    //let layout = Layout::from_size_align((SUB_SIZE * 8) as usize, 8).unwrap();
    //let mut forward_backward: *mut u8 = alloc(layout);
    for i in 0usize..8 {
        if !load_subscription(flash, &mut ret[i], i) {
            break;
        }
    }
    load_emergency_subscription(&mut ret[get_loc_for_channel(0) as usize]);
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
fn load_subscription(flash: &hal::flc::Flc, subscription: &mut Subscription, channel_pos: usize) -> bool {
    let cache: RefCell<[u8; 2048 as usize]> = RefCell::new([0; 2048 as usize]);
    let mut pos: usize = 0;
    let result = flash.check_address(SUB_LOC as u32 + pos as u32);
    if result.is_err() {
        match result.unwrap_err() {
            FlashError::InvalidAddress => {
                console::write_console(b"InvalidAddress\n");
            }
            FlashError::AccessViolation => {
                console::write_console(b"InvalidOperation\n");
            }
            FlashError::NeedsErase => {
                console::write_console(b"NeedsErase\n");
            }
        };
        return false
    }
    unsafe {
        let _ = flash::read_bytes(flash, SUB_LOC as u32 + pos as u32, &mut (*cache.as_ptr()), SUB_SIZE);

        let init = (*cache.as_ptr())[20]; // Should always be non-zero if it's loaded right
        if init == 0 || init == 0xFF {
            return false;
        }

        subscription.location = channel_pos * SUB_SIZE;
        subscription.channel=u32::from_be_bytes((*cache.as_ptr())[pos..pos+4].try_into().unwrap());
        pos += 4;

        let _ = u8::from_be_bytes((*cache.as_ptr())[pos..pos+1].try_into().unwrap());
        let _ = u8::from_be_bytes((*cache.as_ptr())[pos+1..pos+2].try_into().unwrap());
        subscription.start=u64::from_be_bytes((*cache.as_ptr())[pos..pos+8].try_into().unwrap());
        pos += 8;
        subscription.end=u64::from_be_bytes((*cache.as_ptr())[pos..pos+8].try_into().unwrap());
        pos += 8;

        pos += 2; // Lengths

        for j in 0..64 {
            let val = u64::from_be_bytes((*cache.as_ptr())[pos + j*8 ..pos + j*8 + 8].try_into().unwrap());
            if (val == 0 && j > 0) {
                break;
            }
            subscription.forward_pos[j] = val;
        }
        pos += INTERMEDIATE_POS_SIZE * INTERMEDIATE_NUM;

        for j in 0..64 {
            let val = u64::from_be_bytes((*cache.as_ptr())[pos + j*8 ..pos + j*8 + 8].try_into().unwrap());
            if (val == 0 && j > 0) {
                break;
            }
            subscription.backward_pos[j] = val;
        }
        pos += INTERMEDIATE_POS_SIZE * INTERMEDIATE_NUM;
        let encrypted_modulus = UBig::from_be_bytes((*cache.as_ptr())[pos ..pos + 160].try_into().unwrap());
        subscription.n=decrypt_channel_modulus(encrypted_modulus, channel_pos as u32);
        pos += 160;

    }

    true
}


fn decrypt_channel_modulus(encrypted_modulus: UBig, channel_pos: u32) -> ConstDivisor {
    // Choose the resulting 160-byte integer from moduli
    let moduli = include_bytes!("moduli.bin");
    let pos = (channel_pos * 160) as usize;
    let modulus = ConstDivisor::new(UBig::from_be_bytes(moduli[pos..pos+160].try_into().unwrap()));

    // And the same from the private keys
    let private_keys = include_bytes!("privates.bin");
    let pos = (channel_pos * 160) as usize;
    let private_key = UBig::from_be_bytes(private_keys[pos..pos + 160].try_into().unwrap());
    //write_console(private_key.to_string().as_bytes());

    let last_s1 = modulus.reduce(encrypted_modulus);
    let last_s2 =  last_s1.pow(&private_key).residue();
    ConstDivisor::new(last_s2)
}
fn load_emergency_subscription(subscription: &mut Subscription) {
    let cache = include_bytes!("emergency.bin");
    let mut pos = 0;
    subscription.location = 0; // Done as a special case
    subscription.channel = u32::from_be_bytes(cache[pos..pos+4].try_into().unwrap());
    if subscription.channel != 0 {
        return
    }
    pos += 4;
    subscription.start=u64::from_be_bytes(cache[pos..pos+8].try_into().unwrap());
    pos += 8;
    subscription.end=u64::from_be_bytes(cache[pos..pos+8].try_into().unwrap());
    pos += 8;

    let _ = u8::from_be_bytes(cache[pos..pos+1].try_into().unwrap());
    let _ = u8::from_be_bytes(cache[pos+1..pos+2].try_into().unwrap());
    pos += 2;
    
    for j in 0..64 {
        let val = u64::from_be_bytes(cache[pos + j*8 ..pos + j*8 + 8].try_into().unwrap());
        if (val == 0 && j > 0) {
            break;
        }
        subscription.forward_pos[j] = val;
    }
    pos += INTERMEDIATE_POS_SIZE * INTERMEDIATE_NUM;

    for j in 0..64 {
        let val = u64::from_be_bytes(cache[pos + j*8 ..pos + j*8 + 8].try_into().unwrap());
        if (val == 0 && j > 0) {
            break;
        }
        subscription.backward_pos[j] = val;
    }
    pos += INTERMEDIATE_POS_SIZE * INTERMEDIATE_NUM;

    let encrypted_modulus = UBig::from_be_bytes(cache[pos..pos + 160].try_into().unwrap());
    write_console(get_loc_for_channel(0).to_string().as_bytes());

    subscription.n=decrypt_channel_modulus(encrypted_modulus, get_loc_for_channel(0));
    write_console(subscription.n.to_string().as_bytes());
    pos += 160;
}

/**
 * @output The ID of the decoder, as indicated by compilation.
 */
fn get_id() -> u32 {
    env!("DECODER_ID").parse::<u32>().unwrap()
}

pub fn get_channels() -> [u32; 9] {
    let mut ret: [u32; 9] = [0; 9];
    // Get the channels from the environment variable CHANNELS, which is like "1,3,7,8" or something
    let channels = env!("CHANNELS");
    let mut i = 0;
    for channel in channels.split(",") {
        ret[i] = channel.parse::<u32>().unwrap();
        i += 1;
    }
    
    ret
}

/**
* @input The channel ID
* @output The location of the channel in the actual channel list in flash
*/
fn get_loc_for_channel(channel: u32) -> u32 {
    let channels = get_channels();
    write_console(format!("{:?}", channels).as_bytes());
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
    write_console(format!("Panic: {}\n", _info).as_bytes());
    loop {}
}
