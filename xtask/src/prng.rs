const PRNG_SEED: u64 = 1234;
const PRNG_MULTIPLIER: u64 = 0x2545_F491_4F6C_DD1D;
const XORSHIFT_RIGHT_FIRST: u32 = 12;
const XORSHIFT_LEFT: u32 = 25;
const XORSHIFT_RIGHT_SECOND: u32 = 27;
const TOP_BYTE_SHIFT: u32 = 56;

/// xorshift64* generator.
pub struct Prng {
    state: u64,
}

impl Prng {
    pub fn new() -> Self {
        Self::with_seed(PRNG_SEED)
    }

    pub fn with_seed(seed: u64) -> Self {
        Self { state: seed }
    }

    fn next_u64(&mut self) -> u64 {
        self.state ^= self.state >> XORSHIFT_RIGHT_FIRST;
        self.state ^= self.state << XORSHIFT_LEFT;
        self.state ^= self.state >> XORSHIFT_RIGHT_SECOND;
        self.state.wrapping_mul(PRNG_MULTIPLIER)
    }

    pub fn next_byte(&mut self) -> u8 {
        (self.next_u64() >> TOP_BYTE_SHIFT) as u8
    }

    pub(crate) fn next_byte_below(&mut self, limit: u8) -> u8 {
        self.next_byte() % limit
    }
}
