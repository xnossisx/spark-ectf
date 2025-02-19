use crate::{flash, get_id, SUB_LOC};
use core::ffi::c_void;
use core::mem::zeroed;
use core::ops::BitXorAssign;
use crypto_bigint::{Odd, U1024, U8192};
use crypto_bigint::modular::{MontyForm, MontyParams};

type Integer = U1024;

#[derive(Copy)]
#[derive(Clone)]
struct SubStat {
    exists: bool,
    start: u64,
    end: u64,
}

fn get_subscriptions() -> [SubStat; 8] {
    let mut ret: [SubStat; 8] = [SubStat{exists: false, start: 0, end: 0}; 8];
    for i in 0..8 {
        let mut data: [u8; 17] = [0;17];
        let res = flash::read_bytes(((SUB_LOC as usize) + i * 8192) as u32, &mut data, 17);
        ret[i] = SubStat{exists: (data[0] != 0), start: u64::from_be_bytes(data[1..9].split_at(core::mem::size_of::<u64>()).0.try_into().unwrap()),
            end: u64::from_be_bytes(data[9..17].split_at(core::mem::size_of::<u64>()).0.try_into().unwrap())};
    }
    ret
}

struct Subscription {
    mem_location: *const c_void,
    forward_enc: Integer,
    backward_enc: Integer,
    n: Odd<Integer>,
    e: Integer,
    refs: [Integer; 16],
    start: u64,
    end: u64,
}

impl Subscription {
    fn get_ref_exponent(&self, idx: u64) -> Integer {
        let relevant: usize = (idx << 2) as usize;

        Integer::from(&self.refs[relevant as usize])
    }

    /*fn get_forward_key(&self) -> Integer {
        Integer::from(&self.forward_enc | get_id())
    }*/

    fn forward_key_shift(&mut self, frames: u64) {
        let mut forward_new = Integer::from(&self.forward_enc);
        self.key_shift(&mut (forward_new), frames);
        self.forward_enc = Integer::from(forward_new);
        self.start += frames;
    }

    /*fn get_backward_key(&self) -> Integer {
        Integer::from(&self.backward_enc | get_id())
    }*/

    fn backward_key_shift(&mut self, frames: u64) -> Integer {
        let mut backward_new = Integer::from(&self.backward_enc);
        self.key_shift(&mut backward_new, frames);
        backward_new
    }

    fn get_timestamps(&self) -> (u64, u64) {
        (self.start, self.end)
    }

    fn get_total_key(&mut self, frame: u64) -> Integer {
        self.forward_key_shift(frame - &self.start);
        let backward_curr = self.backward_key_shift(&self.end - frame);
        &self.forward_enc | backward_curr
    }

    fn key_shift(&self, key: &mut Integer, frames: u64) {
        let mut bit: u64 = 0;
        let mut exponent: U8192 = U8192::from(&self.e);
        (*key).bitxor_assign(&Integer::from(get_id()));
        let monty_key: MontyForm<32> = MontyForm::new(key, MontyParams::new(self.n));
        while (1 << bit) < frames && bit < 64 {
            if bit % 4 == 0 {
                exponent = U8192::from(&self.get_ref_exponent(bit));
            }
            if (1 << bit) & frames != 0 {
                monty_key.pow(&exponent);
            }
            (exponent,_) = exponent.square_wide();
            bit += 1;
        }
        *key = monty_key.retrieve();
        (*key).bitxor_assign(&Integer::from(get_id()));
    }
}
