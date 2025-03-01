use alloc::vec::Vec;
use core::cell::RefCell;
use crate::{flash, get_hashed_id, get_id, Integer, SUB_LOC, SUB_SIZE};
use core::ffi::c_void;
use core::mem::zeroed;
use core::ops::{BitXorAssign, Not};
use core::ptr::{null, null_mut};
use crypto_bigint::{Encoding, Int, Odd, U1024, U512, U8192};
use crypto_bigint::modular::{montgomery_reduction, MontyForm, MontyParams};
use hal::flc::FLASH_PAGE_SIZE;
use hal::pac::dvs::Mon;

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
    pub(crate) start: u64,
    pub(crate) end: u64,
}

/// Loads subscription listings from flash memory
pub fn get_subscriptions() -> Vec<SubStat> {
    let mut ret: Vec<SubStat> = Vec::new();
    for i in 0usize..8 {
        let mut data: [u8; 17] = [0;17];
        let res = flash::read_bytes((SUB_LOC as u32) + (i as u32) * SUB_SIZE, &mut data, 17);
        if (data[0] == 0) || (data[0] == 0xff) { continue; }
        ret.push(SubStat{
            exists: (data[0] != 0 && data[0] != 0xff),
            start: u64::from_be_bytes(data[1..9].split_at(size_of::<u64>()).0.try_into().unwrap()),
            end: u64::from_be_bytes(data[9..17].split_at(size_of::<u64>()).0.try_into().unwrap())});
    }
    ret
}


#[derive(Copy)]
#[derive(Clone)]
pub struct Subscription {
    pub(crate) n: Odd<Integer>,
    pub(crate) forward_pos: [u64; 16],
    pub(crate) backward_pos: [u64; 16],
    pub(crate) start: u64,
    pub(crate) end: u64,
    pub(crate) channel: u32,
    pub(crate) location: usize
}

impl Subscription {
    pub fn new() -> Subscription {
        Subscription {
            n: Odd::new(Integer::ONE).unwrap(),
            forward_pos: [0; 16],
            backward_pos: [0; 16],
            start: 0,
            end: 0,
            channel: 0,
            location: 0
        }
    }

    pub fn get_intermediate(&self, pos: usize, dir: u64) -> Integer {
        let ref_location = if dir == FORWARD {self.location + 512 + pos * 128} else {self.location + 2560 + pos * 128};
        let ref_location = ref_location as u32;
        let intermediate_buffer: RefCell<[u8; 128]> = RefCell::new([0; 128]);
        unsafe {
            let _ = flash::read_bytes(ref_location, &mut (*intermediate_buffer.as_ptr())[0..128], 128);
            Integer::from_be_bytes((*intermediate_buffer.as_ptr()).try_into().unwrap()).bitxor(get_hashed_id())
        }
    }

    pub fn decode_side(&self, target: u64, dir: u64) -> Integer {
        let pos = if dir == FORWARD {&self.forward_pos} else if dir == BACKWARD {&self.backward_pos} else {return Integer::ZERO};
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

        let mut result: Integer = self.get_intermediate(closest_idx, dir);

        // Takes the result to be the top exponent of a power tower of primes

        let mask_combo = target-closest_idx as u64;
        let mut idx_nibble = 15;
        loop {
            let mask = (1 << (idx_nibble << 2)) * 15;
            let distance = (mask & mask_combo) >> (idx_nibble << 2);
            for i in 0..distance {
                let monty = MontyForm::new(&Integer::from(PRIMES[idx_nibble]), MontyParams::new(self.n)).pow(&result);
                result = monty.retrieve();
            }
            if (idx_nibble == 0) {
                break;
            }
            idx_nibble -= 1;
        }

        result
    }

    pub fn decode(&self, target: Integer, timestamp: u64) -> U512 {
        let forward = self.decode_side(timestamp, FORWARD);
        let backward = self.decode_side(!timestamp, BACKWARD); // Technically passing in 2^64 - timestamp

        let guard = forward.bitxor(&backward);
        (&MontyForm::new(&target, MontyParams::new(self.n)).pow(&guard).retrieve()).into()
    }
}
