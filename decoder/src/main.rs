#![feature(concat_bytes)]
#![no_std]
#![no_main]

use alloc::format;
use hal::trng::Trng;
use core::cell::RefCell;
use core::panic::PanicInfo;
use cortex_m::delay::Delay;
use crypto_bigint::U512;
use ed25519_dalek::{VerifyingKey};
use embedded_alloc::LlffHeap;

type Integer = U512;

type Heap = LlffHeap;

#[global_allocator]
static HEAP: Heap = Heap::empty();

mod console;
mod flash;
mod subscription;
//mod uart;

extern crate alloc;
pub extern crate max7800x_hal as hal;
extern crate aes as encrypt_aes;
const SUB_SPACE: u32 = 8192; /* page length */
const SUB_SIZE: usize = 4+2+64+64+128+8+8+1024+1024;
/* channel # + intermediate lengths + intermediate references + modulus + start + end +
intermediates (1024*2)*/

pub const INTERMEDIATE_NUM: usize = 64;
pub const INTERMEDIATE_LOC: u32 = 1280;
pub const INTERMEDIATE_SIZE: usize = 16;
pub const INTERMEDIATE_POS_SIZE: usize = 8;

type Aes128Ofb = ofb::Ofb<encrypt_aes::Aes128>;
pub 

use hal::entry;
use hal::flc::FlashError;
pub use hal::pac;
use ofb::cipher::{KeyIvInit, StreamCipher};
use flash::flash;
use crate::console::write_console;
use crate::subscription::Subscription;
// you can put a breakpoint on `rust_begin_unwind` to catch panics
// use panic_itm as _; // logs messages over ITM; requires ITM support
// use panic_semihosting as _; // logs messages to the host stderr; requires a debugger
// use cortex_m_semihosting::heprintln; // uncomment to use this for printing through semihosting

/**
 * The location of all of our subscription data on the flash
*/
pub const SUB_LOC: *const u8 = 0x10022000 as *const u8;

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

    let led_r = pins.p2_0.into_input_output();
    let led_g = pins.p2_1.into_input_output();
    let led_b = pins.p2_2.into_input_output();

    // Initialize the trng peripheral
    let trng = Trng::new(p.trng, &mut gcr.reg);


    // Use VDDIOH as the power source for the RGB LED pins (3.0V)
    // Note: This HAL API may change in the future

    // Initialize a delay timer using the ARM SYST (SysTick) peripheral
    let rate = clks.sys_clk.frequency;

    let mut delay = Delay::new(core.SYST, rate);

    // Load subscription from flash memory
    let flash = flash::init(p.flc, clks);
    let mut subscriptions: [Option<Subscription>; 9] = load_subscriptions(&flash);
    
    let divisor = load_verification_key();

    // Fundamental event loop
    loop {
        // Delays to avoid side channel attacks
        let test_val = trng.gen_u32();

        let output = test(test_val, &trng, &mut delay);
        if test_val*test_val == output {
            console::read_resp(&flash, &mut subscriptions, divisor);
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
fn load_subscriptions(flash: &hal::flc::Flc) -> [Option<Subscription>; 9] {
    // Page 1: Modulus, Channel, Start, End, Forward Count, Backward Count
    // Page 2: Forward exponents, Backward exponents
    let mut ret:[Option<Subscription>; 9] = [None; 9];

    //let layout = Layout::from_size_align((SUB_SIZE * 8) as usize, 8).unwrap();
    //let mut forward_backward: *mut u8 = alloc(layout);
    for i in 1usize..get_channels().len() {
        ret[i] = load_subscription(flash, i);
    }
    ret[0] = load_emergency_subscription();
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
fn load_subscription(flash: &hal::flc::Flc, channel_pos: usize) -> Option<Subscription> {
    let mut subscription:Subscription=Subscription::new();
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
        return None
    }
    unsafe {
        let _ = flash::read_bytes(flash, SUB_LOC as u32 + pos as u32, &mut (*cache.as_ptr()), SUB_SIZE);

        let init = (*cache.as_ptr())[20]; // Should always be non-zero if it's loaded right
        if init == 0 || init == 0xFF {
            return None;
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
            if val == 0 && j > 0 {
                break;
            }
            subscription.forward_pos[j] = val;
        }
        pos += INTERMEDIATE_POS_SIZE * INTERMEDIATE_NUM;

        for j in 0..64 {
            let val = u64::from_be_bytes((*cache.as_ptr())[pos + j*8 ..pos + j*8 + 8].try_into().unwrap());
            if val == 0 && j > 0 {
                break;
            }
            subscription.backward_pos[j] = val;
        }
        //pos += INTERMEDIATE_POS_SIZE * INTERMEDIATE_NUM;
        //let mut modulus = (*cache.as_ptr())[pos..pos + 128].try_into().unwrap();
        //decrypt_channel_modulus(&mut modulus, channel_pos as u32);

    }
    drop(cache);
    Some(subscription)
}


fn decrypt_channel_modulus(encrypted_modulus: &mut [u8; 128], channel_pos: u32) {
    // Get the right AES key
    let private_keys = include_bytes!("keys.bin");
    let pos = (channel_pos * 32) as usize;
    let key: [u8; 16] = private_keys[pos + 0..pos + 16].try_into().unwrap();

    let iv: [u8; 16] = private_keys[pos + 16..pos + 32].try_into().unwrap();

    let mut cipher = Aes128Ofb::new(&key.into(), &iv.into());
    cipher.apply_keystream(encrypted_modulus);
}
fn load_emergency_subscription() -> Option<Subscription> {
    let mut subscription:Subscription=Subscription::new();
    let cache = include_bytes!("emergency.bin");
    let mut pos = 0;
    subscription.location = 0; // Done as a special case
    subscription.channel = u32::from_be_bytes(cache[pos..pos+4].try_into().unwrap());
    if subscription.channel != 0 {
        return None;
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
        if val == 0 && j > 0 {
            break;
        }
        subscription.forward_pos[j] = val;
    }
    pos += INTERMEDIATE_POS_SIZE * INTERMEDIATE_NUM;

    for j in 0..64 {
        let val = u64::from_be_bytes(cache[pos + j*8 ..pos + j*8 + 8].try_into().unwrap());
        if val == 0 && j > 0 {
            break;
        }
        subscription.backward_pos[j] = val;
    }
    pos += INTERMEDIATE_POS_SIZE * INTERMEDIATE_NUM;

    let mut modulus = cache[pos..pos + 128].try_into().unwrap();
    decrypt_channel_modulus(&mut modulus, get_loc_for_channel(0));
    pos += 128;
    Some(subscription)
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
* Loads the verification key for elliptic curve signatures
*/
fn load_verification_key() -> VerifyingKey {
    let bytes = include_bytes!("public.bin");
    let attempt = VerifyingKey::from_bytes(bytes);
    if attempt .is_err() {
        console::write_err(format!("{}", attempt.err().unwrap()).as_bytes());
        panic!();
    }
    attempt.unwrap()
}
/**
* @input The channel ID
* @output The location of the channel in the actual channel list in flash
*/
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
    write_console(format!("Panic: {}\n", _info).as_bytes());

    loop {}
}
