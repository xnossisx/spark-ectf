use alloc::alloc::alloc;
use alloc::format;
use alloc::string::ToString;
use core::alloc::Layout;
use core::cmp::{max, min};
use core::mem::MaybeUninit;
use crate::pac::Uart0;
use core::ptr::{null_mut, read_unaligned};
use cortex_m::asm::nop;
use crypto_bigint::{U1024};
use hal::gcr::clocks::{Clock, PeripheralClock};
use hal::gcr::GcrRegisters;
use hal::gpio::{Af1, Pin};
use hal::uart::BuiltUartPeripheral;
use crate::{flash, load_subscription, SUB_LOC, SUB_SIZE};
use crate::subscription::{get_subscriptions, Subscription};

static MAGIC: u8 = b'%';
static mut CONSOLE_INIT: bool = false;
pub(crate) type cons = BuiltUartPeripheral<Uart0, Pin<0, 0, Af1>, Pin<0, 1, Af1>, (), ()>;

pub fn init(
    uart0: Uart0,
    reg: &mut GcrRegisters,
    rx_pin: Pin<0, 0, Af1>,
    tx_pin: Pin<0, 1, Af1>,
    pclk: &Clock<PeripheralClock>,
) -> cons {
    unsafe {
        let ret = hal::uart::UartPeripheral::uart0(uart0, reg, rx_pin, tx_pin)
            .baud(115200)
            .clock_pclk(pclk)
            .parity(hal::uart::ParityBit::None)
            .build();
        if CONSOLE_INIT {
            write_err(&ret, b"Duplicate uart made");
        }
        CONSOLE_INIT = true;
        ret
    }
}

pub fn write_console(console: &cons, bytes: &[u8]) {
    console.write_byte(MAGIC);
    console.write_byte(b'G');
    console.write_byte(((bytes.len() as u16) & 0x00FF) as u8);
    console.write_byte((((bytes.len() as u16) & 0xFF00) >> 8) as u8);
    console.write_bytes(bytes);
}

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

pub fn write_err(console: &cons, bytes: &[u8]) {
    console.write_byte(MAGIC);
    console.write_byte(b'E');
    console.write_byte(((bytes.len() as u16) & 0x00FF) as u8);
    console.write_byte((((bytes.len() as u16) & 0xFF00) >> 8) as u8);
    console.write_bytes(bytes);
}

pub unsafe fn write_async(console: &cons, bytes: &[u8]) {
    console.write_bytes(&bytes);
}

pub fn read_byte(console: &cons) -> u8 {
    console.read_byte()
}

pub fn ack(console: &cons) {
    console.write_bytes(b"%A\x00\x00");
}


// Reads whatever the TV is sending over right now.
pub fn read_resp(subscriptions: &mut [Subscription; 8], console: &cons) {
    let magic = read_byte(console);

    if magic != MAGIC {
        write_console(console, b"that was not magic");
        return;
    }
    let opcode = read_byte(console);
    if (opcode != b'E' && opcode != b'L' && opcode != b'S' && opcode != b'D' && opcode != b'A') {
        write_console(console, b"that was not an opcode");
        write_console(console, &[opcode]);
        return;
    }
    let length: u16 = (read_byte(console) as u16) + ((read_byte(console) as u16) << 8);
    /*let opcode: u8 = b'S';
    let length: u16 = 5;
    */write_console(console, b"bonjour");
    unsafe {
        if opcode == b'L' {
            let subscriptions = get_subscriptions();
            if let Ok(l) = Layout::from_size_align(4usize + subscriptions.len()*20usize, 16) {
                write_console(console, subscriptions.len().to_string().as_bytes());

                let ret = core::slice::from_raw_parts_mut(alloc(l), 4usize + subscriptions.len()*20usize);
                ret[0..4].copy_from_slice(bytemuck::bytes_of(&(subscriptions.len() as u32)));
                //ret[0..4] = bytemuck::try_cast(&(subscriptions.len() as u32)).unwrap();
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
        let layout: Layout;
        if let Ok(l) = Layout::from_size_align(length as usize, 16) {
            layout = l;
        } else {
            write_err(console, b"Alloc error");
            return;
        }


        match opcode {
            b'S' => {
                ack(console);
                let byte_list: &mut [u8] = &mut [0; 256];
                let mut pos = 0;
                for i in 0..((length + 255) >> 8) {
                    //console.read_bytes(get_range(byte_list, i, length));
                    for byte in &mut *byte_list {
                        *byte = console.read_byte();
                    }
                    if (i == 0) {
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
                        });
                        pos = SUB_LOC as usize + channel as usize * SUB_SIZE as usize;
                        write_console(console, format!("Writing to 0x{:x}", pos).as_bytes());
                    } else {
                        pos += 256;
                    }
                    flash::write_bytes(pos as u32, &byte_list, 256).unwrap_or_else(|test| {
                        write_err(console, test);
                    });
                    ack(console);
                }


                let channel: u32 = *bytemuck::from_bytes(&byte_list[2 + 1024 + 128..2 + 1024 + 128 + 4]);
                let pos: usize = SUB_LOC as usize + channel as usize * SUB_SIZE as usize;
                write_console(console, format!{"0x{:x}", pos}.as_bytes());
                load_subscription(&mut subscriptions[pos], console, pos);
                write_comm(console, b"",b'S');
            }
            b'D' => {
                let byte_list: &mut [u8] = core::slice::from_raw_parts_mut(alloc(layout), length as usize);
                write_console(console, ((length + 255) >> 8).to_string().as_bytes());

                for i in 0..((length + 255) >> 8) {
                    //console.read_bytes(get_range(byte_list, i, length));
                    for byte in &mut *byte_list {
                        *byte = console.read_byte();
                    }
                    ack(console);
                }

                let channel: u32 = *bytemuck::from_bytes(&byte_list[0..4]);
                let timestamp: u64 = *bytemuck::from_bytes(&byte_list[4..12]);
                let frame: U1024 = <crate::Integer>::from_be_slice(byte_list[12..140].try_into().unwrap()); // 128 bytes
                let checksum: u32 = *bytemuck::from_bytes(&byte_list[140..144]);
                ack(console);

                let sub = subscriptions.into_iter().filter(|s| s.channel == channel).next().unwrap();

                let decoded = sub.decode(frame, timestamp);
                let ret: [u8; 64] = decoded.to_be_bytes();
                write_comm(console, &ret,b'D');

            }
            _ => return
        }



    }
}

pub fn get_range(list: &mut [u8], page: usize, length: u16) -> &mut [u8] {
    &mut list[page<<8..min((page + 1) << 8, length as usize - (page << 8))]
}
