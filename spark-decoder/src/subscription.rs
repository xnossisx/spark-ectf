use crate::{flash, get_id, Integer, SUB_LOC};
use core::ffi::c_void;
use core::mem::zeroed;
use core::ops::BitXorAssign;
use core::ptr::{null, null_mut};
use crypto_bigint::{Int, Odd, U1024, U8192};
use crypto_bigint::modular::{MontyForm, MontyParams};
use hal::pac::dvs::Mon;

const FORWARD: u64 = 0x1f8c25d4b902e785;
const BACKWARD: u64 = 0xf329d3e6bb90fcc5;

#[derive(Copy)]
#[derive(Clone)]
pub struct SubStat {
    pub(crate) exists: bool,
    pub(crate) start: u64,
    pub(crate) end: u64,
}

pub fn get_subscriptions() -> [SubStat; 8] {
    let mut ret: [SubStat; 8] = [SubStat{exists: false, start: 0, end: 0}; 8];
    for i in 0..8 {
        let mut data: [u8; 17] = [0;17];
        let res = flash::read_bytes(((SUB_LOC as usize) + i * 8192) as u32, &mut data, 17);
        ret[i] = SubStat{exists: (data[0] != 0), start: u64::from_be_bytes(data[1..9].split_at(core::mem::size_of::<u64>()).0.try_into().unwrap()),
            end: u64::from_be_bytes(data[9..17].split_at(core::mem::size_of::<u64>()).0.try_into().unwrap())};
    }
    ret
}


#[derive(Copy)]
#[derive(Clone)]
pub struct Subscription {
    pub(crate) n: Odd<Integer>,
    pub(crate) forward_refs: [Integer; 64],
    pub(crate) back_refs: [Integer; 64],
    pub(crate) forward_pos: [u64; 64],
    pub(crate) backward_pos: [u64; 64],
    pub(crate) start: u64,
    pub(crate) end: u64,
    pub(crate) channel: u32
}

impl Subscription {
    pub fn new() -> Subscription {
        Subscription {
            n: Odd::new(Integer::ONE).unwrap(),
            forward_refs: [Integer::ZERO; 64],
            back_refs: [Integer::ZERO; 64],
            forward_pos: [0; 64],
            backward_pos: [0; 64],
            start: 0,
            end: 0,
            channel: 0
        }
    }
    
    pub fn decode_side(&self, target: u64, dir: u64) -> Integer {
        let refs = if dir == FORWARD {&self.forward_refs} else if dir == BACKWARD {&self.back_refs} else {return Integer::ZERO};
        let pos = if dir == FORWARD {&self.forward_pos} else if dir == BACKWARD {&self.backward_pos} else {return Integer::ZERO};
        let mut closest_pos: u64 = 0;
        let mut closest_intermediate: &Integer = &refs[0];

        for (i, idx_ref) in pos.iter().enumerate() {
            if *idx_ref > target {
                break;
            }
            if *idx_ref > closest_pos as u64 {
                closest_pos = *idx_ref;
                closest_intermediate = &refs[i]
            }
        }

        let mut result: Integer = *closest_intermediate;

        let mut idx_bit = 63;
        loop {
            if (1 << idx_bit) & target > 0 && (1 << idx_bit) & closest_pos == 0 {
                let monty = MontyForm::new(&refs[idx_bit], MontyParams::new(self.n)).pow(&result);
                result = monty.retrieve();
            }
            if (idx_bit == 0) {
                break;
            }
            idx_bit -= 1;
        }

        result
    }

    pub fn decode(&self, target: Integer) -> Integer {
        let forward = self.decode_side(target, FORWARD);
        let backward = self.decode_side(target, BACKWARD);

        let guard = forward.bitxor(backward);
        MontyForm::new(&target, MontyParams::new(self.n)).pow(&guard).retrieve()
    }
}
