use alloc::alloc::alloc;
use core::alloc::Layout;
use core::cmp::{max, min};
use crate::pac::Uart0;
use core::ptr::null_mut;
use cortex_m::asm::nop;
use crypto_bigint::{U1024};
use hal::gcr::clocks::{Clock, PeripheralClock};
use hal::gcr::GcrRegisters;
use hal::gpio::{Af1, Pin};
use hal::uart::BuiltUartPeripheral;
use crate::{flash, SUB_LOC, SUB_SIZE};
use crate::subscription::{get_subscriptions, Subscription};

static MAGIC: u8 = b'%';

static mut CONSOLE_HOOK: *mut BuiltUartPeripheral<Uart0, Pin<0, 0, Af1>, Pin<0, 1, Af1>, (), ()> =
    null_mut();

pub fn init(
    uart0: Uart0,
    reg: &mut GcrRegisters,
    rx_pin: Pin<0, 0, Af1>,
    tx_pin: Pin<0, 1, Af1>,
    pclk: &Clock<PeripheralClock>,
) {
    unsafe {
        if !CONSOLE_HOOK.is_null() { return }
        CONSOLE_HOOK = &mut hal::uart::UartPeripheral::uart0(uart0, reg, rx_pin, tx_pin)
            .baud(115200)
            .clock_pclk(pclk)
            .parity(hal::uart::ParityBit::None)
            .build();
    }
}

pub fn write_console(bytes: &[u8]) {
    unsafe {
        write_comm(bytes, b'G');
    }
}


pub unsafe fn write_comm(bytes: &[u8], code: u8) {
    (*CONSOLE_HOOK).write_byte(MAGIC);
    (*CONSOLE_HOOK).write_byte(code);
    (*CONSOLE_HOOK).write_byte(((bytes.len() as u16) & 0x00FF) as u8);
    (*CONSOLE_HOOK).write_byte((((bytes.len() as u16) & 0xFF00) >> 8) as u8);
    for i in 0..(bytes.len() >> 8) {
        while read_byte() != b'\x07' {
            nop()
        }
        (*CONSOLE_HOOK).write_bytes(&bytes[i<<8..min((i + 1) << 8, bytes.len() - (i << 8))]);
    }
    while read_byte() != b'\x07' {
        nop()
    }
}

pub fn write_err(bytes: &[u8]) {
    unsafe {
        write_comm(bytes, b'E');
    }
}

pub unsafe fn write_async(bytes: &[u8]) {
    (*CONSOLE_HOOK).write_bytes(&bytes);
}

pub fn read_byte() -> u8 {
    unsafe {(*CONSOLE_HOOK).read_byte()}
}

pub fn ack() {
    write_console(b"\x07");
}


// Reads whatever the TV is sending over right now.
pub fn read_resp(subscriptions: &mut [Subscription; 8]) {
    let magic = read_byte();
    if magic != MAGIC {return;}
    let opcode = read_byte();
    if (opcode != b'E' || opcode != b'L' || opcode != b'S' || opcode != b'D' || opcode != b'A') {return;}
    let length: u16 = ((read_byte() as u16) << 8) + (read_byte() as u16);
    unsafe {
        ack();
        if opcode == b'L' {
            let subscriptions = get_subscriptions();
            if let Ok(l) = Layout::from_size_align(4usize + subscriptions.len()*20usize, 16) {
                let ret = core::slice::from_raw_parts_mut(alloc(l), 4usize + subscriptions.len()*20usize);
                ret[0..4].copy_from_slice(bytemuck::bytes_of(&(subscriptions.len() as u32)));
                //ret[0..4] = bytemuck::try_cast(&(subscriptions.len() as u32)).unwrap();
                for i in 0..subscriptions.len() {
                    ret[i*20usize+4..i*20usize+8].copy_from_slice(bytemuck::bytes_of(&(i as i32)));
                    ret[i*20usize+8..i*20usize+16].copy_from_slice(bytemuck::bytes_of(&(subscriptions[i].start)));
                    ret[i*20usize+16..i*20usize+24].copy_from_slice(bytemuck::bytes_of(&(subscriptions[i].end)));
                }
                write_comm(ret,b'L');
                return;
            } else {
                write_err(b"Alloc error");
                return;
            }

        }
        let layout: Layout;
        if let Ok(l) = Layout::from_size_align(length as usize, 16) {
            layout = l;
        } else {
            write_err(b"Alloc error");
            return;
        }
        let byte_list: &mut [u8] = core::slice::from_raw_parts_mut(alloc(layout), length as usize);
        for i in 0..(length >> 8) as usize {
            (*CONSOLE_HOOK).read_bytes(get_range(byte_list, i, length));
            ack();
        }

        match opcode {
            b'S' => {
                let channel: u32 = *bytemuck::from_bytes(&byte_list[0..4]);
                let start: u64 = *bytemuck::from_bytes(&byte_list[4..12]);
                let end: u64 = *bytemuck::from_bytes(&byte_list[12..20]);

                let mut pos: usize = SUB_LOC as usize + channel as usize * SUB_SIZE as usize + 2;

                flash::write_bytes(pos as u32, &byte_list[pos..pos + 1172 as usize], 1172);

                pos = SUB_LOC as usize + (channel as usize * SUB_SIZE as usize) + 8192;
                flash::write_bytes(pos as u32, &byte_list[pos..pos + 16384 as usize], 16384);

                ack();
            }
            b'D' => {
                let channel: u32 = *bytemuck::from_bytes(&byte_list[0..4]);
                let timestamp: u64 = *bytemuck::from_bytes(&byte_list[4..12]);
                let frame: U1024 = <crate::Integer>::from_be_slice(byte_list[12..140].try_into().unwrap()); // 128 bytes
                let checksum: u32 = *bytemuck::from_bytes(&byte_list[140..144]);
                ack();

                let sub = subscriptions.into_iter().filter(|s| s.channel == channel).next().unwrap();

                let decoded = sub.decode(frame, timestamp);
                let ret: [u8; 64] = decoded.to_be_bytes();
                write_comm(&ret,b'D');

            }
            _ => return
        }



    }
}

pub fn get_range(list: &mut [u8], page: usize, length: u16) -> &mut [u8] {
    &mut list[page<<8..min((page + 1) << 8, length as usize - (page << 8))]
}
