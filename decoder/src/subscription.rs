use crate::console::write_console;
use crate::console;
use crate::{decrypt_intermediate, flash, Integer, INTERMEDIATE_LOC, INTERMEDIATE_NUM, INTERMEDIATE_SIZE, SUB_LOC};
use alloc::format;
use alloc::string::ToString;
use alloc::vec::Vec;
use blake3::Hasher;
use crypto_bigint::{Encoding, U512};
use hal;
use hal::flc::{Flc, FLASH_PAGE_SIZE};

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
        let mut data: [u8; 32] = [0; 32];

        let _res = flash::read_bytes(flash, (SUB_LOC as u32) + (i as u32) * FLASH_PAGE_SIZE, &mut data, 32);
        console::write_console(&data);
        if (data[20] == 0) || (data[20] == 0xff) { continue; }
        ret.push(SubStat {
            exists: data[20] != 0 && data[20] != 0xff,
            channel: u32::from_be_bytes(data[0..4].split_at(size_of::<u32>()).0.try_into().unwrap()),
            start: u64::from_be_bytes(data[4..12].split_at(size_of::<u64>()).0.try_into().unwrap()),
            end: u64::from_be_bytes(data[12..20].split_at(size_of::<u64>()).0.try_into().unwrap())
        });
    }
    ret
}

#[derive(Clone, Debug)]
#[derive(Copy)]
pub struct Subscription {
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
            forward_pos: [0; INTERMEDIATE_NUM],
            backward_pos: [0; INTERMEDIATE_NUM],
            start: 0,
            end: 0,
            channel: 0,
            location: 0,
            curr_frame: 0
        }
    }

    pub fn get_intermediate(&self, flash: &hal::flc::Flc, pos: usize, dir: u64) -> u128 {
        if self.location == 0 { // Emergency channel
            let sub_bytes = include_bytes!("emergency.bin");
            let intermediate_pos = INTERMEDIATE_LOC as usize + pos * INTERMEDIATE_SIZE + (if dir == FORWARD {0} else {1024});
            write_console(b"Getting intermediate...");
            write_console(&sub_bytes[intermediate_pos..intermediate_pos+16]);
            return u128::from_be_bytes(sub_bytes[intermediate_pos..intermediate_pos+16].try_into().unwrap());
        }
        
        let ref_location = if dir == FORWARD
            {self.location + (INTERMEDIATE_LOC as usize) + pos * INTERMEDIATE_SIZE }
        else
            {self.location + (INTERMEDIATE_LOC as usize) + 1024 + pos * INTERMEDIATE_SIZE };
        let ref_location = ref_location as u32;
        let mut intermediate_buffer: [u8; INTERMEDIATE_SIZE] = [0; INTERMEDIATE_SIZE];
        let _ = flash::read_bytes(flash, ref_location, &mut intermediate_buffer, INTERMEDIATE_SIZE);
        write_console(format!("Intermediate {}",pos).as_bytes());
        u128::from_be_bytes(intermediate_buffer)
    }

    pub fn decode_side(&self, flash: &Flc, target: u64, dir: u64) -> U512 {
        let pos = if dir == FORWARD {&self.forward_pos} else if dir == BACKWARD {&self.backward_pos} else {return U512::from(0u32)};
        let mut closest_pos: u64 = 0;
        let mut closest_idx: usize = 0;


        // Finds the intermediate closest to the target
        for (i, idx_ref) in pos.iter().enumerate() {
            if *idx_ref > target || (*idx_ref == 0 && i != 0) {
                break;
            }
            if *idx_ref > closest_pos {
                closest_pos = *idx_ref;
                closest_idx = i;
            }
        }

        let compressed_enc: u128 = self.get_intermediate(&flash, closest_idx, dir);
        let mut compressed= decrypt_intermediate(compressed_enc, self.channel);
        write_console(compressed.to_string().as_bytes());
        // The number of trailing zeros helps determine what step the intermediate is at! Perfect.
        let mut idx = trailing_zeroes_special(closest_pos) - 1;
        loop {
            let mask = 1 << idx;
            if mask & target != 0 { // Determines if the bit needs to be flipped on.
                console::write_console(idx.to_string().as_bytes());
                console::write_console(format!("{}", compressed).to_string().as_bytes());
                compressed = Self::compress(compressed, idx as u8);
            }
            if idx == 0 {
                break;
            }
            idx -= 1;
        }
        //let mut res = self.n.reduce(u128::from_be_bytes(compressed.to_be_bytes())).pow(&UBig::from(PRIMES[idx])).residue();
        // (&MontyForm::new(&Integer::from(&compressed), MontyParams::new(self.n))).pow(&Integer::from(PRIMES[idx])).retrieve()
        let mut output_bytes: [u8; 64] = [0; 64];
        output_bytes[48..].copy_from_slice(&compressed.to_be_bytes());
        <Integer>::from_be_bytes(output_bytes)
    }


    pub fn compress(n: u128, section: u8) -> u128 {
        let mut hasher: Hasher = Hasher::new();
        hasher.update(&section.to_be_bytes());
        hasher.update(&n.to_be_bytes());
        let binding = hasher.finalize();
        let (_, res): (&[u8], &[_]) = binding.as_bytes().split_at(size_of::<u128>());
        u128::from_be_bytes(res.try_into().unwrap())
    }
    const BIG_BYTES: [u8; 64] =  [92, 244, 129, 255, 230, 241, 27, 64, 141, 102, 255, 242, 62, 90, 184,
        39, 179, 61, 229, 42, 43, 60, 236, 180, 17, 81, 0, 19, 40, 237, 9, 31, 190, 96, 11, 35, 242, 31,
        191, 50, 123, 176, 19, 168, 38, 117, 144, 128, 85, 72, 55, 123, 175, 222, 187, 108, 70, 122, 249,
        95, 86, 175, 58, 231];
    // Gets the lowest bit that is on: e.g. returns "2" from "0100".
    pub fn decode(&self, flash: &Flc, frame: U512, timestamp: u64) -> U512 {
        let forward = self.decode_side(flash, timestamp, FORWARD);
        let backward = self.decode_side(flash, !timestamp, BACKWARD); // Technically passing in 2^64 - timestamp
        let guard:U512 = forward ^ backward;
        let (product, _) = guard.split_mul(&<Integer>::from_be_bytes(Self::BIG_BYTES));
        frame ^ product

        // (&(&(&MontyForm::new(&target, MontyParams::new(self.n))).pow(&Integer::from(65537u32)).retrieve()).bitxor(&guard)).into()
        //U512::from(guard.log2_bits())
    }
}
pub fn trailing_zeroes_special(target: u64) -> usize {
    if target == 0 {return INTERMEDIATE_NUM;}
    target.trailing_zeros() as usize
}