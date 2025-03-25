use crate::{get_loc_for_channel, Integer, SUB_SPACE};
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
use cortex_m::peripheral::SYST;
use ed25519_dalek::{Digest, DigestVerifier, Sha512, Signature, VerifyingKey};
use hal::gcr::clocks::{Clock, PeripheralClock};
use hal::gcr::GcrRegisters;
use hal::gpio::{Af1, Pin};
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
        while read_byte() != b'\x25' {
            nop()
        }
        console().read_byte();
        console().read_byte();
        console().read_byte();

        console().write_bytes(&bytes[i<<8..min((i + 1) << 8, bytes.len())]);
    }
    while read_byte() != b'\x25' {
        nop()
    }
    console().read_byte();
    console().read_byte();
    console().read_byte();
}

/// Writes a properly formatted error message to the console without expecting a response (mostly debug)
/// @param console: The UART console reference the message is sent through.
/// @param bytes: The list of bytes sent through.
pub fn write_err(bytes: &[u8]) {
    console().write_byte(MAGIC);
    console().write_byte(b'E');
    console().write_byte(((bytes.len() as u16) & 0x00FF) as u8);
    console().write_byte((((bytes.len() as u16) & 0xFF00) >> 8) as u8);
    console().write_bytes(bytes);
}

/// Writes a message to the console without expecting a response
/// @param console: The UART console reference the message is sent through.
pub unsafe fn write_async(bytes: &[u8]) {
    console().write_bytes(&bytes);
}

/// Sends an ACK signal to the console
/// @param console: The UART console reference the byte will be received from.
pub fn read_byte() -> u8 {
    let res = console().read_byte();
    // write_console(&[res]);
    res
}

/// Sends an ACK signal to the console
/// @param console: The UART console reference the ACK is sent through.
pub fn ack() {
    console().write_bytes(b"%A\x00\x00");
}


/// Reads whatever the TV is sending over right now, and responds to it.
/// @param subscriptions: A list of subscriptions.
/// @param console: A reference to the UART console.
pub fn read_resp(flash: &hal::flc::Flc, subscriptions: &mut [Option<Subscription>; 9], verifier: VerifyingKey) {
    // Check that the first byte is the magic byte %; otherwise, we return
    let header: &mut [u8] = &mut [0; 4];
    for byte in &mut *header {
        *byte = read_byte();
    }
    let magic = header[0];

    if magic != MAGIC {
        write_console(b"that was not magic");
        write_console(&[magic]);
        return;
    }

    // Reads and checks the validity of the opcode
    let opcode = header[1];
    if opcode != b'E' && opcode != b'L' && opcode != b'S' && opcode != b'D' && opcode != b'A' {
        write_console(b"that was not an opcode");
        write_console(&[opcode]);
        return;
    }

    // Reads the length value
    let length: u16 = (header[2] as u16) + ((header[3] as u16) << 8);
    // write_console(b"bonjour");
    unsafe {
        match opcode {
            b'L' => {
                ack();
                // Responds to the list command by getting the subscriptions...
                let subscriptions = get_subscriptions(flash);
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

                    if i == 0 {
                        let channel_id = ((byte_list[0] as u32) << 24) +
                            ((byte_list[1] as u32) << 16) +
                            ((byte_list[2] as u32) << 8) + (byte_list[3] as u32);
                        if channel_id == 0 {
                            write_err(b"Cannot be given emergency subscription");
                            return;
                        }
                        // Casts the first 4 bytes to the channel value
                        channel = get_loc_for_channel(channel_id);

                        pos = SUB_LOC as u32 + (channel * SUB_SPACE);
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

                subscriptions[channel as usize] = load_subscription(flash, channel as usize);

                if subscriptions[channel as usize].is_none() {
                    write_err(b"Failed to load subscription");
                } else {
                    write_console(b"Success");
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
                        write_console(b"done");
                        break;
                    }
                }

                if sub.is_none() {
                    write_console(channel.to_string().as_bytes());
                    write_err(b"No channel for this frame!");
                    return;
                }

                if (sub.unwrap().start > timestamp) {
                    write_comm(b"fail", b'D');
                    return;
                } else if (sub.unwrap().end <= timestamp) {
                    write_err(b"Timestamp is too late");
                    return;
                }

                if (sub.unwrap().curr_frame > timestamp) {
                    write_console(b"Timestamp is out of order!!! This violates security requirement #3. Billions of decoders must fail.");
                    write_comm(b"fail",b'D');
                    return;
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
                    return;
                }

                // Return the decoded bytes to the TV
                write_comm(&ret,b'D');
                dealloc(byte_list.as_mut_ptr(),layout);

            }
            other => {
                write_err(format!("Unknown opcode: {}", other).as_bytes());
                return
            }
        }



    }
}

pub fn get_range(list: &mut [u8], page: usize, length: u16) -> &mut [u8] {
    &mut list[page<<8..min((page + 1) << 8, length as usize - (page << 8))]
}
