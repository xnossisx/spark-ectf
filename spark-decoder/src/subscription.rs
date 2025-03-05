use alloc::vec::Vec;
use core::cell::RefCell;
use crate::{flash, get_hashed_id, Integer, INTERMEDIATE_LOC, INTERMEDIATE_NUM, INTERMEDIATE_SIZE, SUB_LOC, SUB_SIZE, SUB_SPACE};
use core::ffi::c_void;
use core::hash::Hash;
use core::mem::zeroed;
use core::ops::{BitXorAssign, Not};
use core::ptr::{null, null_mut};
use blake3::Hasher;
use crypto_bigint::{Encoding, Int, Odd, U1024, U128, U512, U8192};
use crypto_bigint::modular::{montgomery_reduction, MontyForm, MontyParams};
use hal;
use hal::flc::{Flc, FLASH_PAGE_SIZE};
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
    pub(crate) channel: u32,
    pub(crate) start: u64,
    pub(crate) end: u64,
}

/// Loads subscription listings from flash memory
pub fn get_subscriptions(flash: &hal::flc::Flc) -> Vec<SubStat> {
    let mut ret: Vec<SubStat> = Vec::new();
    for i in 0usize..8 {
        let mut data: [u8; 22] = [0;22];
        let res = flash::read_bytes(flash, (SUB_LOC as u32) + (i as u32) * (SUB_SIZE as u32), &mut data, 22);
        if (data[0] == 0) || (data[0] == 0xff) { continue; }
        ret.push(SubStat{
            exists: (data[0] != 0 && data[0] != 0xff),
            channel: u32::from_be_bytes(data[0..4].split_at(size_of::<u32>()).0.try_into().unwrap()),
            start: u64::from_be_bytes(data[6..14].split_at(size_of::<u64>()).0.try_into().unwrap()),
            end: u64::from_be_bytes(data[14..22].split_at(size_of::<u64>()).0.try_into().unwrap())});
    }
    ret
}


#[derive(Copy)]
#[derive(Clone)]
pub struct Subscription {
    pub(crate) n: Odd<Integer>,
    pub(crate) forward_pos: [u64; INTERMEDIATE_NUM as usize],
    pub(crate) backward_pos: [u64; INTERMEDIATE_NUM as usize],
    pub(crate) start: u64,
    pub(crate) end: u64,
    pub(crate) channel: u32,
    pub(crate) location: usize,
    pub(crate) curr_frame: u64
}

impl Subscription {
    pub fn new() -> Subscription {
        Subscription {
            n: Odd::new(Integer::ONE).unwrap(),
            forward_pos: [0; INTERMEDIATE_NUM],
            backward_pos: [0; INTERMEDIATE_NUM],
            start: 0,
            end: 0,
            channel: 0,
            location: 0,
            curr_frame: 0
        }
    }

    pub fn get_intermediate(&self, flash: &hal::flc::Flc, pos: usize, dir: u64) -> Integer {
        let ref_location = if dir == FORWARD
            {self.location + (INTERMEDIATE_LOC as usize) + pos * INTERMEDIATE_SIZE as usize}
        else
            {self.location + (INTERMEDIATE_LOC as usize) + 1024 + pos * INTERMEDIATE_SIZE as usize};
        let ref_location = ref_location as u32;
        let intermediate_buffer: RefCell<[u8; INTERMEDIATE_NUM as usize]> = RefCell::new([0; INTERMEDIATE_NUM]);
        unsafe {
            let _ = flash::read_bytes(flash, ref_location, &mut (*intermediate_buffer.as_ptr())[0..INTERMEDIATE_SIZE], INTERMEDIATE_SIZE as usize);
            Integer::from_be_bytes((*intermediate_buffer.as_ptr()).try_into().unwrap()).bitxor(&Integer::from(get_hashed_id()))
        }
    }

    pub fn decode_side(&self, flash: &Flc, target: u64, dir: u64) -> Integer {
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

        let mut result: Integer = self.get_intermediate(&flash, closest_idx, dir);
        // Takes the result to be the top exponent of a power tower of primes

        let mut idx = INTERMEDIATE_NUM;
        loop {
            let mask = 1 << idx;
            if mask & target != 0 {
                let monty = MontyForm::new(&Integer::from(PRIMES[idx]), MontyParams::new(self.n)).pow(&U128::from(Self::compress(result, idx)));
                result = monty.retrieve();
            }
            if (idx == 0) {
                break;
            }
            idx -= 1;
        }
        result
    }

    const adjoint: u128 = 2 ** 128 - 1;
    pub fn compress(n: Integer, section: u32) -> u128 {
        let hasher: Hasher = blake3::Hasher::new();
        hasher.hash(section.to_be_bytes().as_mut_ref());
        hasher.hash(&mut n.to_be_bytes());
        let compressed = u128::from_be_bytes(hasher.finalize().as_bytes()) & Self::adjoint;
        compressed
    }

    pub fn decode(&self, flash: &hal::flc::Flc, target: Integer, timestamp: u64) -> U512 {
        let forward = self.decode_side(flash, FORWARD, timestamp);
        let backward = self.decode_side(flash, !timestamp, BACKWARD); // Technically passing in 2^64 - timestamp

        let guard = forward.bitxor(&backward);
        (&MontyForm::new(&target, MontyParams::new(self.n)).pow(&guard).retrieve()).into()
    }
}
