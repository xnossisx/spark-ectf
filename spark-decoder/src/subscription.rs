use core::ffi::c_void;
use core::ops::{BitXor, BitXorAssign};
use rug::Integer;
use crate::get_id;

struct Subscription {
    mem_location: *const c_void,
    forward_enc: Integer,
    backward_enc: Integer,
    n: Integer,
    e: Integer,
    refs: [Integer; 16],
    start: u64,
    end: u64
}

impl Subscription {
    fn get_ref_exponent(&self, idx: u64) -> Integer {
        let relevant: usize = (idx << 2) as usize;

        Integer::from(&self.refs[relevant as usize])
    }

    fn get_forward_key(&self) -> Integer {
        Integer::from(&self.forward_enc | get_id())
    }

    fn forward_key_shift(&mut self, frames: u64) {
        let mut forward_new = Integer::from(&self.forward_enc);
        self.key_shift(&mut (forward_new), frames);
        self.forward_enc = Integer::from(forward_new);
        self.start += frames;
    }

    fn get_backward_key(&self) -> Integer {
        Integer::from(&self.backward_enc | get_id())
    }

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
        let backward_curr = self.backward_key_shift(&self.end-frame);
        &self.forward_enc | backward_curr
    }

    fn key_shift(&self, key: &mut Integer, frames: u64) {
        let mut bit: u64 = 0;
        let mut exponent: Integer = Integer::from(&self.e);
        key.bitxor_assign(get_id());
        while (1 << bit) < frames && bit < 64 {
            if bit % 4 == 0 {
                exponent = self.get_ref_exponent(bit);
            }
            if (1 << bit) & frames != 0 {
                key.secure_pow_mod_mut(&exponent, &self.n);
            }
            exponent = exponent.square();
            bit += 1;
        }
        key.bitxor_assign(get_id());
    }
}

