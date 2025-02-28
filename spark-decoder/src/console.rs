use crate::get_loc_for_channel;
use crate::pac::Uart0;
use crate::subscription::{get_subscriptions, Subscription};
use crate::{flash, load_subscription, SUB_LOC, SUB_SIZE};
use alloc::alloc::alloc;
use alloc::format;
use alloc::string::ToString;
use core::alloc::Layout;
use core::cmp::min;
use cortex_m::asm::nop;
use crypto_bigint::U1024;
use hal::gcr::clocks::{Clock, PeripheralClock};
use hal::gcr::GcrRegisters;
use hal::gpio::{Af1, Pin};
use hal::uart::BuiltUartPeripheral;

static MAGIC: u8 = b'%';

pub(crate) type cons = BuiltUartPeripheral<Uart0, Pin<0, 0, Af1>, Pin<0, 1, Af1>, (), ()>;

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
    pclk: &Clock<PeripheralClock>,
) -> cons {
    unsafe {
hal::uart::UartPeripheral::uart0(uart0, reg, rx_pin, tx_pin)
            .baud(115200)
            .clock_pclk(pclk)
            .parity(hal::uart::ParityBit::None)
            .build()
    }
}

/// Sends a properly formatted debug message to the console.
/// @param console: The UART console reference the message is sent through.
/// @param bytes: The list of bytes sent through.
pub fn write_console(console: &cons, bytes: &[u8]) {
    console.write_byte(MAGIC);
    console.write_byte(b'G');
    console.write_byte(((bytes.len() as u16) & 0x00FF) as u8);
    console.write_byte((((bytes.len() as u16) & 0xFF00) >> 8) as u8);
    console.write_bytes(bytes);
}

/// Sends a properly formatted message to the console.
/// @param console: The UART console reference the message is sent through.
/// @param bytes: The list of bytes sent through.
/// @param code: The opcode of the operation.
pub fn write_comm(console: &cons, bytes: &[u8], code: u8) {
    console.write_byte(MAGIC);
    console.write_byte(code);
    console.write_byte(((bytes.len() as u16) & 0x00FF) as u8);
    console.write_byte((((bytes.len() as u16) & 0xFF00) >> 8) as u8);

    for i in 0..((bytes.len() + 255) >> 8) {
        while read_byte(console) != b'\x25' {
            nop()
        }
        console.read_byte();
        console.read_byte();
        console.read_byte();

        console.write_bytes(&bytes[i<<8..min((i + 1) << 8, bytes.len())]);
    }
    while read_byte(console) != b'\x25' {
        nop()
    }
    console.read_byte();
    console.read_byte();
    console.read_byte();
}

/// Writes a properly formatted error message to the console without expecting a response (mostly debug)
/// @param console: The UART console reference the message is sent through.
/// @param bytes: The list of bytes sent through.
pub fn write_err(console: &cons, bytes: &[u8]) {
    console.write_byte(MAGIC);
    console.write_byte(b'E');
    console.write_byte(((bytes.len() as u16) & 0x00FF) as u8);
    console.write_byte((((bytes.len() as u16) & 0xFF00) >> 8) as u8);
    console.write_bytes(bytes);
}

/// Writes a message to the console without expecting a response
/// @param console: The UART console reference the message is sent through.
pub unsafe fn write_async(console: &cons, bytes: &[u8]) {
    console.write_bytes(&bytes);
}

/// Sends an ACK signal to the console
/// @param console: The UART console reference the byte will be received from.
pub fn read_byte(console: &cons) -> u8 {
    console.read_byte()
}

/// Sends an ACK signal to the console
/// @param console: The UART console reference the ACK is sent through.
pub fn ack(console: &cons) {
    console.write_bytes(b"%A\x00\x00");
}


/// Reads whatever the TV is sending over right now, and responds to it.
/// @param subscriptions: A list of subscriptions.
/// @param console: A reference to the UART console.
pub fn read_resp(subscriptions: &mut [Subscription; 8], console: &cons) {
    // Check that the first byte is the magic byte %; otherwise, we return
    let magic = read_byte(console);

    if magic != MAGIC {
        write_console(console, b"that was not magic");
        return;
    }

    // Reads and checks the validity of the opcode
    let opcode = read_byte(console);
    if (opcode != b'E' && opcode != b'L' && opcode != b'S' && opcode != b'D' && opcode != b'A') {
        write_console(console, b"that was not an opcode");
        write_console(console, &[opcode]);
        return;
    }

    // Reads the length value
    let length: u16 = (read_byte(console) as u16) + ((read_byte(console) as u16) << 8);
    write_console(console, b"bonjour");
    unsafe {
        if opcode == b'L' {
            // Responds to the list command by getting the subscriptions...
            let subscriptions = get_subscriptions();
            if let Ok(l) = Layout::from_size_align(4usize + subscriptions.len()*20usize, 16) {
                write_console(console, subscriptions.len().to_string().as_bytes());

                // Allocating the return space...
                let ret = core::slice::from_raw_parts_mut(alloc(l), 4usize + subscriptions.len()*20usize);
                ret[0..4].copy_from_slice(bytemuck::bytes_of(&(subscriptions.len() as u32)));
                //ret[0..4] = bytemuck::try_cast(&(subscriptions.len() as u32)).unwrap();
                // Casts parts of the subscription data to the listing
                for i in 0..subscriptions.len() {
                    ret[i*20usize+4..i*20usize+8].copy_from_slice(bytemuck::bytes_of(&(i as i32)));
                    ret[i*20usize+8..i*20usize+16].copy_from_slice(bytemuck::bytes_of(&(subscriptions[i].start)));
                    ret[i*20usize+16..i*20usize+24].copy_from_slice(bytemuck::bytes_of(&(subscriptions[i].end)));
                }
                write_comm(console, ret,b'L');
                return;
            } else {
                write_err(console, b"Alloc error");
                return;
            }

        }


        match opcode {
            b'S' => {
                // Acknowledges the data transfer
                ack(console);
                // Initializes the array of bytes that will hold the packets
                let byte_list: &mut [u8] = &mut [0; 256];
                let mut channel = 0;
                let mut pos = 0;
                for i in 0..((length + 255) >> 8) {
                    // Reads bytes from console
                    for byte in &mut *byte_list {
                        *byte = console.read_byte();
                    }
                    if i == 0 {
                        // Casts the first 4 bytes to the channel value
                        let channel: u32 = *bytemuck::try_from_bytes(&byte_list[0..4]).unwrap_or_else(|test| {
                            match test {
                                bytemuck::PodCastError::AlignmentMismatch => {
                                    write_console(console, b"AlignmentMismatch");
                                }
                                bytemuck::PodCastError::SizeMismatch => {
                                    write_console(console, b"SizeMismatch");
                                }
                                bytemuck::PodCastError::TargetAlignmentGreaterAndInputNotAligned => {
                                    write_console(console, b"TargetAlignmentGreaterAndInputNotAligned");
                                }
                                bytemuck::PodCastError::OutputSliceWouldHaveSlop => {
                                    write_console(console, b"OutputSliceWouldHaveSlop");
                                }
                            }
                            &0
                        }));
                        pos = SUB_LOC as usize + (channel as usize * SUB_SIZE as usize);
                    } else {
                        pos += 256;
                    }
                    // Writes data to the flash
                    flash::write_bytes(pos as u32, &byte_list, 256, console).unwrap_or_else(|test| {
                        write_err(console, test);
                    });
                    ack(console);
                }
                // Load subscription and send debug information
                load_subscription(&mut subscriptions[channel as usize], console, channel as usize);
                write_comm(console, b"",b'S');
            }
            b'D' => {
                // Initializes a layout
                let layout: Layout;
                if let Ok(l) = Layout::from_size_align(length as usize, 16) {
                    layout = l;
                } else {
                    write_err(console, b"Alloc error");
                    return;
                }
                // Allocates space for the bytes
                let byte_list: &mut [u8] = core::slice::from_raw_parts_mut(alloc(layout), length as usize);
                write_console(console, ((length + 255) >> 8).to_string().as_bytes());

                // Receives bytes
                for i in 0..((length + 255) >> 8) {
                    //console.read_bytes(get_range(byte_list, i, length));
                    for byte in &mut *byte_list {
                        *byte = console.read_byte();
                    }
                    ack(console);
                }

                // Splits up the data
                let channel: u32 = *bytemuck::from_bytes(&byte_list[0..4]);
                let timestamp: u64 = *bytemuck::from_bytes(&byte_list[4..12]);
                let frame: U1024 = <crate::Integer>::from_be_slice(byte_list[12..140].try_into().unwrap()); // 128 bytes
                let checksum: u32 = *bytemuck::from_bytes(&byte_list[140..144]);
                ack(console);

                // Get the relevant subscription, and use it to decode
                let sub = subscriptions.into_iter().filter(|s| s.channel == channel).next().unwrap();
                let decoded = sub.decode(frame, timestamp);
                let ret: [u8; 64] = decoded.to_be_bytes();

                // Return the decoded bytes to the TV
                write_comm(console, &ret,b'D');

            }
            _ => return
        }



    }
}

pub fn get_range(list: &mut [u8], page: usize, length: u16) -> &mut [u8] {
    &mut list[page<<8..min((page + 1) << 8, length as usize - (page << 8))]
}
