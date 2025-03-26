use crate::{get_subscription_for_channel, test, Integer, SUB_SPACE};
use crate::pac::Uart0;
use crate::subscription::{get_subscriptions, Subscription};
use crate::{flash, load_subscription, SUB_LOC};
use alloc::alloc::{alloc, dealloc};
use alloc::format;
use alloc::string::ToString;
use core::alloc::Layout;
use core::cmp::min;
use core::mem::MaybeUninit;
use cortex_m::asm::nop;
use cortex_m::delay::Delay;
use ed25519_dalek::{Digest, DigestVerifier, Sha512, Signature, VerifyingKey};
use hal::flc::Flc;
use hal::gcr::clocks::{Clock, PeripheralClock};
use hal::gcr::GcrRegisters;
use hal::gpio::{Af1, Pin};
use hal::trng::Trng;
use hal::uart::BuiltUartPeripheral;

static MAGIC: u8 = b'%';

pub(crate) type Cons = BuiltUartPeripheral<Uart0, Pin<0, 0, Af1>, Pin<0, 1, Af1>, (), ()>;

// Core reference to our flash (initially uninitialized)
const CONSOLE_HANDLE: MaybeUninit<Cons> = MaybeUninit::uninit();

/**
 * Gets a reference to the console
 * @output: An immutable console reference
 */
pub fn console() -> &'static Cons {
    unsafe { CONSOLE_HANDLE.assume_init_ref() }
}



/// Initializes the UART0 console.
/// @param uart0: A reference to the uart 0 system.
/// @param reg: A reference to the general control registers.
/// @param rx_pin: A reference to the receiving pin
/// @param tx_pin: A reference to the transmitting pin
/// @param pclk: A reference to a peripheral clock
pub fn init(
    uart0: Uart0,
    reg: &mut GcrRegisters,
    rx_pin: Pin<0, 0, Af1>,
    tx_pin: Pin<0, 1, Af1>,
    pclk: &Clock<PeripheralClock>
) {
    CONSOLE_HANDLE.write(hal::uart::UartPeripheral::uart0(uart0, reg, rx_pin, tx_pin)
        .baud(115200)
        .clock_pclk(pclk)
        .parity(hal::uart::ParityBit::None)
        .build());
}

/// Sends a properly formatted debug message to the console.
/// @param console: The UART console reference the message is sent through.
/// @param bytes: The list of bytes sent through.
pub fn write_console(bytes: &[u8]) {
    console().write_byte(MAGIC);
    console().write_byte(b'G');
    console().write_byte(((bytes.len() as u16) & 0x00FF) as u8);
    console().write_byte((((bytes.len() as u16) & 0xFF00) >> 8) as u8);
    console().write_bytes(bytes);
}

/// Sends a properly formatted message to the console.
/// @param console: The UART console reference the message is sent through.
/// @param bytes: The list of bytes sent through.
/// @param code: The opcode of the operation.
pub fn write_comm(bytes: &[u8], code: u8) {
    console().write_byte(MAGIC);
    console().write_byte(code);
    console().write_byte(((bytes.len() as u16) & 0x00FF) as u8);
    console().write_byte((((bytes.len() as u16) & 0xFF00) >> 8) as u8);

    for i in 0..((bytes.len() + 255) >> 8) {
        eat_ack();

        console().write_bytes(&bytes[i<<8..min((i + 1) << 8, bytes.len())]);
    }
    eat_ack();
}

/// Writes a properly formatted error message to the console without expecting a response (mostly debug)
/// @param console: The UART console reference the message is sent through.
/// @param bytes: The list of bytes sent through.
pub fn write_err(bytes: &[u8]) {
    console().write_byte(MAGIC);
    console().write_byte(b'E');
    console().write_byte(((bytes.len() as u16) & 0x00FF) as u8);
    console().write_byte((((bytes.len() as u16) & 0xFF00) >> 8) as u8);
    eat_ack();

    console().write_bytes(bytes);
    eat_ack();
}

pub fn eat_ack() {
    while read_byte() != b'\x25' {
        nop()
    }
    console().read_byte();
    console().read_byte();
    console().read_byte();
}

/// Sends an ACK signal to the console
/// @param console: The UART console reference the byte will be received from.
pub fn read_byte() -> u8 {
    console().read_byte()
}

/// Sends an ACK signal to the console
/// @param console: The UART console reference the ACK is sent through.
pub fn ack() {
    console().write_bytes(b"%A\x00\x00");
}


/// Reads whatever the TV is sending over right now, and responds to it.
/// @param subscriptions: A list of subscriptions.
/// @param console: A reference to the UART console.
pub fn read_resp(flash: &hal::flc::Flc, subscriptions: &mut [Option<Subscription>; 9], verifier: VerifyingKey, trng: &Trng, delay: &mut Delay) {
    // Check that the first byte is the magic byte %; otherwise, we return
    let header: &mut [u8] = &mut [0; 4];
    for byte in &mut *header {
        *byte = read_byte();
    }
    let magic = header[0];

    if magic != MAGIC {
        write_console(b"that was not magic");
        write_console(header);
        return;
    }

    // Reads and checks the validity of the opcode
    let opcode = header[1];
    if opcode != b'E' && opcode != b'L' && opcode != b'S' && opcode != b'D' && opcode != b'A' {
        write_console(b"that was not an opcode");
        write_console(header);
        return;
    }

    // Reads the length value
    let length: u16 = (header[2] as u16) + ((header[3] as u16) << 8);

    // Delays to avoid side channel attacks
    let test_val = trng.gen_u32();

    let output = test(test_val, &trng, delay);
    if test_val*test_val == output {
    } else {
        write_err(b"Integrity check failed");
    }
    unsafe {
        match opcode {
            b'L' => {
                ack();
                // Responds to the list command by getting the subscriptions...
                let subscriptions = get_subscriptions(subscriptions);
                if let Ok(l) = Layout::from_size_align(4usize + subscriptions.len()*20usize, 16) {

                    // Allocating the return space...
                    let ret = core::slice::from_raw_parts_mut(alloc(l), 4usize + subscriptions.len()*20usize);
                    ret[0..4].copy_from_slice(bytemuck::bytes_of(&(subscriptions.len() as u32)));
                    //ret[0..4] = bytemuck::try_cast(&(subscriptions.len() as u32)).unwrap();
                    // Casts parts of the subscription data to the listing
                    for i in 0..subscriptions.len() {
                        ret[i*20usize+4..i*20usize+8].copy_from_slice(bytemuck::bytes_of(&(subscriptions[i].channel)));
                        ret[i*20usize+8..i*20usize+16].copy_from_slice(bytemuck::bytes_of(&(subscriptions[i].start)));
                        ret[i*20usize+16..i*20usize+24].copy_from_slice(bytemuck::bytes_of(&(subscriptions[i].end)));
                    }
                    write_comm(ret,b'L');
                    dealloc(ret.as_mut_ptr(), l);
                    return;
                } else {
                    write_err(b"Alloc error");
                    return;
                }
            }
            b'S' => {
                // Acknowledges the data transfer
                // Initializes the array of bytes that will hold the packets
                let byte_list: &mut [u8] = &mut [0; 256];
                let mut channel = 0;
                let mut pos = 0;
                ack();
                for i in 0..((length + 255) >> 8) {
                    // Reads bytes from console
                    for byte in &mut *byte_list {
                        *byte = read_byte();
                    }
                    write_console(b"Read bytes");
                    if i == 0 {
                        // Casts the first 4 bytes to the channel value
                        let channel_id = ((byte_list[0] as u32) << 24) +
                            ((byte_list[1] as u32) << 16) +
                            ((byte_list[2] as u32) << 8) + (byte_list[3] as u32);
                        if channel_id == 0 {
                            write_err(b"Cannot be given emergency subscription");
                            return;
                        }

                        // Turns the channel ID into an index
                        let maybe_channel = get_subscription_for_channel(channel_id, subscriptions);
                        if maybe_channel.is_none() {
                            write_err(b"Channel does not exist");
                            return;
                        }
                        channel = maybe_channel.unwrap();
                        write_console(format!("Channel: {}", channel).as_bytes());

                        // Erases the flash space

                        pos = SUB_LOC as u32 + ((channel - 1) * SUB_SPACE); // Push back by one to deal with emergency channel
                        flash.erase_page(pos).unwrap_or_else(|test| {
                            write_err(flash::map_err(test).as_bytes());
                        });
                    } else {
                        pos += 256;
                    }
                    // Writes data to the flash
                    flash::write_bytes(flash, pos, &byte_list, 256).unwrap_or_else(|err| {
                        write_err(err);
                    });
                    ack();
                }
                // Test to see if it was actually written
/*                let dst = &mut [0; 256];
                let test = flash::read_bytes(flash, SUB_LOC as u32 + (channel * SUB_SPACE), dst, 256);
                if test.is_err() {
                    write_err(test.unwrap_err());
                }
                write_console(dst);*/
                
                // Load subscription and send debug information

                subscriptions[channel as usize] = load_subscription(flash, channel as usize - 1); // Push back by one to deal with emergency channel

                if subscriptions[channel as usize].is_none() {
                    write_err(b"Failed to load subscription");
                }
                
                write_comm(b"",b'S');
            }
            b'D' => {
                // Initializes a layout
                let layout: Layout;
                if let Ok(l) = Layout::from_size_align(length as usize, 16) {
                    layout = l;
                } else {
                    write_err(b"Alloc error");
                    return;
                }
                // Allocates space for the bytes
                let byte_list: &mut [u8] = core::slice::from_raw_parts_mut(alloc(layout), length as usize);
                ack();
                // Receives bytes
                for _ in 0..((length + 255) >> 8) {
                    for byte in &mut *byte_list {
                        *byte = read_byte();
                    }
                    ack();
                }
                
                // Return the decoded bytes to the TV
                match decode_subroutine(flash, subscriptions, verifier, layout, &byte_list) {
                    Some(value) => write_comm(&value,b'D'),
                    None => return,
                };
                dealloc(byte_list.as_mut_ptr(), layout);

            }
            b'A' => {
                // Acknowledge
            }
            other => {
                write_err(format!("Unknown opcode: {}", other).as_bytes());
                return
            }
        }
    }
}

fn decode_subroutine(flash: &Flc, subscriptions: &mut [Option<Subscription>; 9], verifier: VerifyingKey, layout: Layout, byte_list: &&mut [u8]) -> Option<[u8; 64]> {
    // Splits up the data
    let channel: u32 = u32::from_be_bytes(*&byte_list[0..4].try_into().unwrap());
    let timestamp: u64 = u64::from_be_bytes(*&byte_list[4..12].try_into().unwrap());
    let frame: Integer = <crate::Integer>::from_be_slice(&byte_list[76..140]); // 64 bytes
    let signature: Signature = Signature::from_slice(&byte_list[12..76]).unwrap();
    //let checksum: u32 = u32::from_be_bytes(*&byte_list[140..144].try_into().unwrap());

    // Get the relevant subscription, and use it to decode
    let mut sub: Option<Subscription> = None;
    for sub_i in subscriptions {
        if sub_i.is_some() && sub_i.clone().unwrap().channel == channel {
            sub = Some(sub_i.clone().unwrap());
            break;
        }
    }

    if sub.is_none() {
        write_console(&byte_list[0..4]);
        write_console(channel.to_string().as_bytes());
        write_err(b"No channel for this frame!");
        return None;
    }

    if sub.unwrap().start > timestamp {
        write_comm(b"fail", b'D');
        return None;
    } else if sub.unwrap().end <= timestamp {
        write_err(b"Timestamp is too late");
        return None;
    }

    if sub.unwrap().curr_frame > timestamp {
        write_console(b"Timestamp is out of order!!! This violates security requirement #3. Billions of decoders must fail.");
        write_comm(b"fail", b'D');
        return None;
    }
    sub.as_mut().unwrap().curr_frame = timestamp + 1;

    // write_console(format!("Channel: {}\n", channel).as_bytes());
    // write_console(format!("timestamp: {}\n", timestamp).as_bytes());

    let decoded = sub.unwrap().decode(flash, frame, timestamp);

    let ret: [u8; 64] = decoded.to_be_bytes();

    // write_console(&ret);

    let ret_digest = Sha512::default().chain_update(ret);

    let chan_bytes = channel.to_be_bytes();
    let verifier_context = verifier.with_context(&chan_bytes).unwrap();
    let evaluation = verifier_context.verify_digest(ret_digest, &signature);

    if evaluation.is_err() {
        write_console(b"Key verification failed - frame spoofing may be happening!");
        write_err(b"Not the actual frame");
        return None;
    }
    Some(ret)
}