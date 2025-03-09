use alloc::boxed::Box;
use alloc::format;
use alloc::string::ToString;
use crate::{flash, INTERMEDIATE_LOC, INTERMEDIATE_NUM, INTERMEDIATE_SIZE, SUB_LOC, SUB_SIZE};
use alloc::vec::Vec;
use blake3::Hasher;
use core::cell::RefCell;
use core::hash::Hash;
use core::mem;
use core::ops::BitXor;
use dashu_int::fast_div::ConstDivisor;
use dashu_int::modular::Reduced;
use hal;
use hal::flc::Flc;
use dashu_int::{UBig, Word};
use crate::console::write_console;

/// Indicate test keys to protect against tampering
const FORWARD: u64 = 0x1f8c25d4b902e785;
const BACKWARD: u64 = 0xf329d3e6bb90fcc5;


// First 64 primes after 1024
const PRIMES: [u32; 64] = [1031,1033,1039,1049,1051,1061,1063,1069,1087,1091,1093,1097,1103,1109,1117,1123,1129,1151,1153,1163,1171,1181,1187,1193,1201,1213,1217,1223,1229,1231,1237,1249,1259,1277,1279,1283,1289,1291,1297,1301,1303,1307,1319,1321,1327,1361,1367,1373,1381,1399,1409,1423,1427,1429,1433,1439,1447,1451,1453,1459,1471,1481,1483,1487];

/// Represents a subscription listing
#[derive(Copy)]
#[derive(Clone)]
pub struct SubStat {
    pub(crate) exists: bool,
    pub(crate) channel: u32,
    pub(crate) start: u64,
    pub(crate) end: u64,
}

/// Loads subscription listings from flash memory
pub fn get_subscriptions(flash: &hal::flc::Flc) -> Vec<SubStat> {
    let mut ret: Vec<SubStat> = Vec::new();
    for i in 0usize..8 {
        let mut data: [u8; 22] = [0;22];
        let _res = flash::read_bytes(flash, (SUB_LOC as u32) + (i as u32) * (SUB_SIZE as u32), &mut data, 22);
        if (data[0] == 0) || (data[0] == 0xff) { continue; }
        ret.push(SubStat{
            exists: (data[0] != 0 && data[0] != 0xff),
            channel: u32::from_be_bytes(data[0..4].split_at(size_of::<u32>()).0.try_into().unwrap()),
            start: u64::from_be_bytes(data[6..14].split_at(size_of::<u64>()).0.try_into().unwrap()),
            end: u64::from_be_bytes(data[14..22].split_at(size_of::<u64>()).0.try_into().unwrap())});
    }
    ret
}


pub struct Subscription {
    pub(crate) n: ConstDivisor,
    pub(crate) forward_pos: [u64; INTERMEDIATE_NUM],
    pub(crate) backward_pos: [u64; INTERMEDIATE_NUM],
    pub(crate) start: u64,
    pub(crate) end: u64,
    pub(crate) channel: u32,
    pub(crate) location: usize,
    pub(crate) curr_frame: u64
}

impl Subscription {
    pub fn new() -> Subscription {
        Subscription {
            n: ConstDivisor::new(UBig::from(1u32)),
            forward_pos: [0; INTERMEDIATE_NUM],
            backward_pos: [0; INTERMEDIATE_NUM],
            start: 0,
            end: 0,
            channel: 0,
            location: 0,
            curr_frame: 0
        }
    }

    pub fn set_modulus(&mut self, n: UBig) {
        self.n = ConstDivisor::new(UBig::from(n));
    }

    pub fn get_intermediate(&self, flash: &hal::flc::Flc, pos: usize, dir: u64) -> UBig {
        if self.location == 0 { // Emergency channel
            let sub_bytes = include_bytes!("emergency.bin");
            let intermediate_pos = INTERMEDIATE_LOC as usize + pos * INTERMEDIATE_SIZE + if dir == FORWARD {0} else {1024};
            return UBig::from_be_bytes(sub_bytes[intermediate_pos..intermediate_pos+16].try_into().unwrap(), );
        }
        
        let ref_location = if dir == FORWARD
            {self.location + (INTERMEDIATE_LOC as usize) + pos * INTERMEDIATE_SIZE }
        else
            {self.location + (INTERMEDIATE_LOC as usize) + 1024 + pos * INTERMEDIATE_SIZE };
        let ref_location = ref_location as u32;
        let mut intermediate_buffer: Box<[u8; INTERMEDIATE_SIZE]> = Box::new([0; INTERMEDIATE_SIZE]);
        unsafe {
            let _ = flash::read_bytes(flash, ref_location, &mut (*intermediate_buffer)[0..INTERMEDIATE_SIZE], INTERMEDIATE_SIZE);
            UBig::from_be_bytes(&(*intermediate_buffer))
        }
    }

    pub fn decode_side(&self, flash: &Flc, target: u64, dir: u64) -> UBig {
        let pos = if dir == FORWARD {&self.forward_pos} else if dir == BACKWARD {&self.backward_pos} else {return UBig::from(0u32) };
        let mut closest_pos: u64 = 0;
        let mut closest_idx: usize = 0;


        // Finds the intermediate closest to the target
        for (i, idx_ref) in pos.iter().enumerate() {
            if *idx_ref > target {
                break;
            }
            if *idx_ref > closest_pos {
                closest_pos = *idx_ref;
                closest_idx = i;
            }
        }

        write_console(format!("closest pos: {}\n", closest_pos).as_bytes());
        let intermediate: &UBig = &self.get_intermediate(&flash, closest_idx, dir);
        // The number of trailing zeros helps determine what step the intermediate is at! Perfect.
        let mut result: UBig = UBig::from(1u32);
        // Takes the result to be the top exponent of a power tower of primes
        write_console(b"expanded\n");

        let mut base: UBig = UBig::from(1u32);

        let mut idx = INTERMEDIATE_NUM - 1;
        loop {
            let mask = 1 << idx;
            if mask & target != 0 {
                base = PRIMES[idx].try_into().unwrap();
                result = Self::compress(self.n.reduce(target).pow(&base).residue(), idx as u32);
            }
            if (idx == 0) {
                break;
            }
            idx -= 1;
        }
        result
    }


    pub fn compress(n: UBig, section: u32) -> UBig {
        let mut hasher: Hasher = Hasher::new();
        hasher.update(&section.to_be_bytes());
        hasher.update(&n.to_be_bytes());
        let binding = hasher.finalize();
        let (res, _): (&[u8], &[_]) = binding.as_bytes().split_at(size_of::<Word>());
        UBig::from_be_bytes(res.try_into().unwrap())
    }

    // Gets the lowest bit that is on: e.g. returns "2" from "0100".
    pub fn decode(&self, flash: &hal::flc::Flc, target: UBig, timestamp: u64) -> UBig {
        write_console(self.n.to_string().as_bytes());
        let forward = self.decode_side(flash, timestamp, FORWARD);
        let backward = self.decode_side(flash, !timestamp, BACKWARD); // Technically passing in 2^64 - timestamp

        let guard = forward.bitxor(backward);

        self.n.reduce(target).pow(&UBig::from(65537u32)).residue().bitxor(guard)
    }
}
pub fn trailing_zeroes_special(target: u64) -> usize {
    if target == 0 { return 0; }
    target.trailing_zeros() as usize
}