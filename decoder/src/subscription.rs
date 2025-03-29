use crate::{decrypt_intermediate, flash, Integer, INTERMEDIATE_LOC, INTERMEDIATE_NUM, INTERMEDIATE_SIZE};
use alloc::vec::Vec;
use blake3::Hasher;
use crypto_bigint::{Encoding, U512};
use hal;
use hal::flc::Flc;
use crate::console::write_console;

/// Indicate test keys to protect against tampering
const FORWARD: u64 = 0x1f8c25d4b902e785;
const BACKWARD: u64 = 0xf329d3e6bb90fcc5;

/// Represents a subscription listing
#[derive(Copy)]
#[derive(Clone)]
pub struct SubStat {
    pub(crate) channel: u32,
    pub(crate) start: u64,
    pub(crate) end: u64,
}

/// Loads subscription listings from flash memory
pub fn get_subscriptions(subscriptions: &mut [Option<Subscription>; 9]) -> Vec<SubStat> {
    let mut ret: Vec<SubStat> = Vec::new();
    for i in 1usize..9 {
        let sub = subscriptions[i];
        if sub.is_none() { continue; }
        else { ret.push(SubStat { channel: sub.unwrap().channel, start: sub.unwrap().start, end: sub.unwrap().end }); }
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

    pub fn get_intermediate(&self, flc: &Flc, pos: usize, dir: u64) -> u128 {
        if self.location == 0 { // Emergency channel
            let sub_bytes = include_bytes!("emergency.bin");
            let intermediate_pos = INTERMEDIATE_LOC as usize + pos * INTERMEDIATE_SIZE + (if dir == FORWARD {0} else {1024});
            return u128::from_be_bytes(sub_bytes[intermediate_pos..intermediate_pos+16].try_into().unwrap());
        }
        
        let ref_location = if dir == FORWARD
            {self.location + (INTERMEDIATE_LOC as usize) + pos * INTERMEDIATE_SIZE }
        else
            {self.location + (INTERMEDIATE_LOC as usize) + 1024 + pos * INTERMEDIATE_SIZE };
        let ref_location = ref_location as u32;
        let mut intermediate_buffer: [u8; INTERMEDIATE_SIZE] = [0; INTERMEDIATE_SIZE];
        let _ = flash::read_bytes(flc, ref_location, &mut intermediate_buffer, INTERMEDIATE_SIZE);
        u128::from_be_bytes(intermediate_buffer)
    }

    /// Decodes one part of the symmetric key for each frame.
    /// Encryption and decryption are symmetric, but the main difference is that different intermediates are used 
    /// @param flc The flash controller, as always
    /// @param target The timestamp, possibly inverted
    /// @param dir Whether the key we're working with is forwards or backwards
    /// @return A part of the key we need
    pub fn decode_side(&self, flc: &Flc, target: u64, dir: u64) -> U512 {
        // The wackiness here is another way to avoid fault injection
        let pos = if dir == FORWARD {&self.forward_pos} else if dir == BACKWARD {&self.backward_pos} else {return U512::from(0u32)};
        let mut closest_pos: u64 = 0;
        let mut closest_idx: usize = 0;
        
        // Finds the intermediate for the value the closest above the target (note that these are sorted)
        for (i, idx_ref) in pos.iter().enumerate() {
            // If the intermediate's reference value is above the target, or if it's just 0 (but not with the first one!), we already have the right intermediate
            if *idx_ref > target || (*idx_ref == 0 && i != 0) {
                break;
            }
            // If we find a bigger intermediate, switch to it
            if *idx_ref > closest_pos {
                closest_pos = *idx_ref;
                closest_idx = i;
            }
        }

        // Gets the intermediate from the closest index
        let intermediate: u128 = self.get_intermediate(flc, closest_idx, dir);
        let mut hashed_int = decrypt_intermediate(intermediate, self.channel);
        // The number of trailing zeros helps determine what iterations the value needs! Perfect.
        let mut idx = trailing_zeroes_special(closest_pos) - 1;
        loop {
            let mask = 1 << idx;
            if mask & target != 0 { // Determines if it needs to be flipped on this time.
                hashed_int = Self::hash(hashed_int, idx as u8);
            }
            if idx == 0 {
                break;
            }
            idx -= 1;
        }
        let mut output_bytes: [u8; 64] = [0; 64];
        output_bytes[48..].copy_from_slice(&hashed_int.to_be_bytes());
        <Integer>::from_be_bytes(output_bytes)
    }

    /// Hashes a number and keeps it at the right size, using half of BLAKE3's output
    /// @param n The value being hashed
    /// @param section Based on the step in the key generation process (and the intermediate size), this number ranges from 0 to 63
    /// @return The hashed number
    pub fn hash(n: u128, section: u8) -> u128 {
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
    
    /// Manages the decoding process, combining the forward & backward keys with extra hash data and returning the decoded frame
    /// @param flc The flash controller
    /// @param frame The individual encrypted frame
    /// @param timestamp The timestamp of the frame
    /// @return Returns the decoded frame
    pub fn decode(&self, flc: &Flc, frame: U512, timestamp: u64) -> U512 {
        let forward = self.decode_side(flc, timestamp, FORWARD);
        let backward = self.decode_side(flc, !timestamp, BACKWARD); // Technically passing in 2^64 - timestamp
        let guard:U512 = forward ^ backward;
        let mut product: [u8; 64] = [0u8; 64];
        Hasher::new().update(&guard.to_be_bytes()).update(&Self::BIG_BYTES).finalize_xof().fill(&mut product);
        frame ^ Integer::from_be_bytes(product)
    }
}
/// A helper function calculating how many iterations are required in decode_side.
/// @param target The "intermediate" being used
/// @return The number of iterations necessary to fully calculate the key part
pub fn trailing_zeroes_special(target: u64) -> usize {
    if target == 0 {return INTERMEDIATE_NUM;}
    target.trailing_zeros() as usize
}