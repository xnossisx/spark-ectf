#![no_std]
#![no_main]

use alloc::format;
use hal::trng::Trng;
use core::cell::RefCell;
use core::panic::PanicInfo;
use blake3::Hasher;
use cortex_m::delay::Delay;
use crypto_bigint::U512;
use ed25519_dalek::VerifyingKey;
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
const REQUIRED_MEMORY: u32 = 4 + 8 + 8 + 2 + (64 * 8 * 2);
/* channel # + start + end + length checks + forward key indices + backward key indices */

pub const INTERMEDIATE_NUM: usize = 64;
pub const INTERMEDIATE_LOC: u32 = 1280;
pub const INTERMEDIATE_SIZE: usize = 16;
pub const INTERMEDIATE_POS_SIZE: usize = 8;

type Aes128Ofb = ofb::Ofb<encrypt_aes::Aes128>;

use hal::entry;
use hal::flc::{FlashError};
pub use hal::pac;
use ofb::cipher::{KeyIvInit, StreamCipher};
use crate::console::{write_console, write_err};
use crate::subscription::Subscription;


// The location of all of our subscription data on the flash
pub const SUB_LOC: *const u8 = 0x10036000 as *const u8;

#[entry]
fn main() -> ! {

    // Initialize peripherals, general control registers, oscillator, and clocks
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
    let pins = hal::gpio::Gpio2::new(p.gpio2, &mut gcr.reg).split();
    // Initialize a delay resource
    let rate = clks.sys_clk.frequency;
    let mut delay = Delay::new(core.SYST, rate);
    let mut led_r = pins.p2_0.into_input_output();
    let mut led_g = pins.p2_1.into_input_output();led_r.set_power_vddioh();
    led_r.set_power_vddioh();
    led_g.set_power_vddioh();
    //Spark!
    led_r.set_high();
    led_g.set_high();
    
    // Configure UART to host computer with 115200 8N1 settings
    let rx_pin = gpio0_pins.p0_0.into_af1();
    let tx_pin = gpio0_pins.p0_1.into_af1();
    let _ = &console::init(p.uart0, &mut gcr.reg, rx_pin, tx_pin, &clks.pclk);

    // Initializes the heap
    {
        use core::mem::MaybeUninit;
        const HEAP_SIZE: usize = 2048;
        static mut HEAP_MEM: [MaybeUninit<u8>; HEAP_SIZE] = [MaybeUninit::uninit(); HEAP_SIZE];
        unsafe { HEAP.init(HEAP_MEM.as_ptr() as usize, HEAP_SIZE) }
    }

    // Initialize the TRNG (True Random Number Generator) peripheral
    let trng = Trng::new(p.trng, &mut gcr.reg);

    // Load subscription from flash memory
    let flash = flash::init(p.flc, clks);
    let mut subscriptions: [Option<Subscription>; 9] = load_subscriptions(&flash);
    let divisor = load_verification_key();

    // Fundamental event loop
    loop {
        console::read_resp(&flash, &mut subscriptions, divisor, &trng, &mut delay);
    }
}

/// This function is used where the risk of serious data corruption is high, thereby allowing us to detect interference
/// @param trng A reference to the TRNG resource
/// @param delay A reference to a delay resource, used to give time for attacks to disrupt the data
/// @return A value indicating success or failure
pub fn test(trng: &Trng, delay: &mut Delay) -> bool {
    let test_val = trng.gen_u32();
    let output = test_2(test_val, &trng, delay);
    if test_val * test_val == output {
        true
    } else {
        write_err(b"Integrity check failed");
        delay.delay_ms(4500);
        false
    }
}

/// Subroutine that performs the delayed calculation
/// Refer to pub fn test just above this
fn test_2(scan: u32, trng: &Trng, delay: &mut Delay) -> u32 {
    let ret = scan.clone();
    delay.delay_us(5u32 + (trng.gen_u32() & 255));
    ret*ret
}

///Reads all subscriptions from the flash
///Acts as a wrapper to load_subscription
///@param flash A handle to the flash system
///@return A list of possible subscriptions
fn load_subscriptions(flc: &hal::flc::Flc) -> [Option<Subscription>; 9] {
    // Page 1: Modulus, Channel, Start, End, Forward Count, Backward Count
    // Page 2: Forward exponents, Backward exponents
    let mut ret:[Option<Subscription>; 9] = [None; 9];

    for i in 1usize..9 {
        ret[i] = load_subscription(flc,i - 1);
    }
    ret[0] = load_emergency_subscription();
    ret
}

/// Reads a non-emergency subscription from the flash
/// Acts as a wrapper to load_subscription
/// Reports errors to the console
/// @param flash A handle to the flash system
/// @param channel_pos A value from 0 to 7 representing an index of the flash memory
/// @return The potential subscription now loaded into memory
fn load_subscription(flc: &hal::flc::Flc, channel_pos: usize) -> Option<Subscription> {
    let mut subscription: Subscription = Subscription::new();
    let cache: RefCell<[u8; 2048 as usize]> = RefCell::new([0; 2048 as usize]);
    let address: usize = SUB_LOC as usize + (channel_pos * SUB_SPACE as usize);

    // Ensures that the address is valid
    let result = flc.check_address(address as u32);
    if result.is_err() {
        match result.unwrap_err() {
            FlashError::InvalidAddress => {
                write_err(b"InvalidAddress\n");
            }
            FlashError::AccessViolation => {
                write_err(b"InvalidOperation\n");
            }
            FlashError::NeedsErase => {
                write_err(b"NeedsErase\n");
            }
        };
        return None
    }
    unsafe {
        let _ = flash::read_bytes(flc, address as u32, &mut (*cache.as_ptr()), REQUIRED_MEMORY as usize);

        let init = (*cache.as_ptr())[20]; // Should always be non-zero if it's loaded right
        if init == 0 || init == 0xFF {
            write_console(b"SubscriptionError");
            return None;
        }
        let mut pos = 0;

        subscription.location = address;
        subscription.channel=u32::from_be_bytes((*cache.as_ptr())[pos..pos+4].try_into().unwrap());
        pos += 4;

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
    }
    drop(cache);
    Some(subscription)
}

/// Converts an intermediate in flash to the actual intermediate using AES
/// @param encrypted_int The encrypted intermediate
/// @param channel The channel ID of the intermediate
/// @return The decrypted intermediate
fn decrypt_intermediate(encrypted_int: u128, channel: u32) -> u128 {
    // Get the right AES key by getting the right channel
    let channel_pos = get_decrypt_loc_for_channel(channel);
    let mut copy = u128::to_be_bytes(encrypted_int);
    let private_keys = include_bytes!("keys.bin");
    let pos = (channel_pos * 32) as usize;
    
    // Separate out the different parts of the key
    let key: [u8; 16] = private_keys[pos + 0.. pos + 16].try_into().unwrap();
    let iv: [u8; 16] = private_keys[pos + 16.. pos + 32].try_into().unwrap();

    // Initialize the cipher, decode the key, and return it
    let mut cipher = Aes128Ofb::new(&key.into(), &iv.into());
    cipher.apply_keystream(&mut copy);
    u128::from_be_bytes(copy)
}

/// Loads the one emergency subscription from program memory
/// @return The emergency subscription, if it's valid, else None
fn load_emergency_subscription() -> Option<Subscription> {
    let mut subscription:Subscription=Subscription::new();
    let cache = include_bytes!("emergency.bin");
    let mut pos = 0;
    subscription.location = 0; // Done as a special case
    subscription.channel = u32::from_be_bytes(cache[pos..pos+4].try_into().unwrap());
    if subscription.channel != 0 {
        write_console(b"why");
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
    Some(subscription)
}

/// Gets the list of channels 
/// @return A list of 17 possible used channels
pub fn get_channels() -> [u32; 17] {
    let mut ret: [u32; 17] = [0; 17];
    // Get the channels from the environment variable CHANNELS, which is like "1,3,7,8" or something
    let channels = env!("CHANNELS");
    ret[0] = 0;
    let mut i  = 1;
    for channel in channels.split(",") {
        let pos = channel.parse::<u32>().unwrap();
        if i < ret.len() {
            ret[i] = pos;
        }
        i += 1;
    }
    
    ret
}

/// Loads the verification key for elliptic curve signatures
/// @return The verification key
fn load_verification_key() -> VerifyingKey {
    let bytes = include_bytes!("public.bin");
    let attempt = VerifyingKey::from_bytes(bytes);
    if attempt.is_err() {
        console::write_err(format!("{}", attempt.err().unwrap()).as_bytes());
        panic!();
    }
    attempt.unwrap()
}

/// Helps find a subscription in flash
/// @param channel The channel ID
/// @return The location of the channel in the actual channel list in flash
fn get_decrypt_loc_for_channel(channel: u32) -> u32 {
    let channels = get_channels();
    for i in 0..channels.len() {
        if channels[i] == channel {
            return i as u32;
        }
    }
    0
}

/// Selects the right channel from the subscription list
/// @param channel: The channel ID.
/// @param subscriptions: The mutable list of subscriptions.
/// @return Gives the right position.
fn get_subscription_for_channel(channel: u32, subscriptions: &mut [Option<Subscription>; 9]) -> Option<u32> {
    for i in 0..subscriptions.len() as u32 {
        match subscriptions[i as usize] {
            None => {
                return Some(i);
            }
            Some(sub) => {
                if sub.channel == channel {
                    return Some(i);
                }
            }
        }
    }
    None
}


/// Allows for simple panicking. 
#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {    write_err(format!("Panic: {}\n", _info).as_bytes()); }
}
