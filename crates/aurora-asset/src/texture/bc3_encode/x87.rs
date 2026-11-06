//! The arithmetic AuroraAsset.dll's encoder does on the x87 FPU. An expression is evaluated in
//! double precision and rounded to `f32` only where the DLL stores it, and several of its loops
//! run with the FPU set to round toward zero, so every operation and store in them truncates.
//! In round-to-nearest code a single operation on two `f32` values rounds the same as plain
//! `f32` arithmetic, so only longer expressions there go through [`X87`].

// Derived from AuroraAsset.dll for interoperability: x87 double precision and rounding control.

use std::ops::{Add, Div, Mul, Sub};

/// The FPU rounding mode a stretch of the DLL's code runs with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Fpu {
    RoundToNearest,
    RoundTowardZero,
}

/// An intermediate result on the x87 stack.
#[derive(Debug, Clone, Copy)]
pub(super) struct X87 {
    value: f64,
    fpu: Fpu,
}

impl Fpu {
    /// Loads an `f32` (or a small integer) onto the stack; loading is exact.
    pub(super) fn load(self, value: f32) -> X87 {
        X87 {
            value: f64::from(value),
            fpu: self,
        }
    }
}

impl X87 {
    /// Rounds the result to `f32`, as the DLL does when it stores it to memory.
    pub(super) fn store(self) -> f32 {
        let nearest = self.value as f32;
        match self.fpu {
            Fpu::RoundToNearest => nearest,
            Fpu::RoundTowardZero if f64::from(nearest).abs() > self.value.abs() => {
                step_toward_zero_f32(nearest)
            }
            Fpu::RoundTowardZero => nearest,
        }
    }

    /// The result of an operation whose correctly rounded value is `nearest` and whose exact value
    /// is `nearest + error`.
    fn rounded(self, nearest: f64, error: f64) -> X87 {
        let rounded_away_from_zero =
            nearest != 0.0 && error != 0.0 && (error < 0.0) != (nearest < 0.0);
        let value = if self.fpu == Fpu::RoundTowardZero && rounded_away_from_zero {
            step_toward_zero_f64(nearest)
        } else {
            nearest
        };
        X87 {
            value,
            fpu: self.fpu,
        }
    }
}

fn step_toward_zero_f64(value: f64) -> f64 {
    if value > 0.0 {
        value.next_down()
    } else {
        value.next_up()
    }
}

fn step_toward_zero_f32(value: f32) -> f32 {
    if value > 0.0 {
        value.next_down()
    } else {
        value.next_up()
    }
}

impl Add for X87 {
    type Output = X87;
    fn add(self, other: X87) -> X87 {
        let sum = self.value + other.value;
        // Knuth's two-sum: the exact rounding error of the addition.
        let other_part = sum - self.value;
        let error = (self.value - (sum - other_part)) + (other.value - other_part);
        self.rounded(sum, error)
    }
}

impl Sub for X87 {
    type Output = X87;
    fn sub(self, other: X87) -> X87 {
        self + X87 {
            value: -other.value,
            fpu: other.fpu,
        }
    }
}

impl Mul for X87 {
    type Output = X87;
    fn mul(self, other: X87) -> X87 {
        let product = self.value * other.value;
        let error = self.value.mul_add(other.value, -product);
        self.rounded(product, error)
    }
}

impl Div for X87 {
    type Output = X87;
    fn div(self, other: X87) -> X87 {
        let quotient = self.value / other.value;
        let remainder = (-quotient).mul_add(other.value, self.value);
        let error = remainder / other.value;
        self.rounded(quotient, error)
    }
}

macro_rules! with_f32_operand {
    ($trait:ident, $method:ident) => {
        impl $trait<f32> for X87 {
            type Output = X87;
            fn $method(self, other: f32) -> X87 {
                let other = self.fpu.load(other);
                $trait::$method(self, other)
            }
        }
    };
}

with_f32_operand!(Add, add);
with_f32_operand!(Sub, sub);
with_f32_operand!(Mul, mul);
with_f32_operand!(Div, div);

impl PartialEq<f32> for X87 {
    fn eq(&self, other: &f32) -> bool {
        self.value == f64::from(*other)
    }
}

impl PartialOrd<f32> for X87 {
    fn partial_cmp(&self, other: &f32) -> Option<std::cmp::Ordering> {
        self.value.partial_cmp(&f64::from(*other))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CHOP: Fpu = Fpu::RoundTowardZero;
    const NEAREST: Fpu = Fpu::RoundToNearest;

    #[test]
    fn rounds_once_at_the_store_not_after_every_operation() {
        let value = f32::from_bits(0x3cf2_4732);
        assert_eq!(value * 255.0 + 0.5, f32::from_bits(0x4100_aa76));
        assert_eq!(
            (NEAREST.load(value) * 255.0 + 0.5).store(),
            f32::from_bits(0x4100_aa75)
        );
    }

    #[test]
    fn truncating_store_never_rounds_up() {
        let two_thirds = 2.0f32 / 3.0;
        assert_eq!((NEAREST.load(2.0) / 3.0).store(), two_thirds);
        assert_eq!((CHOP.load(2.0) / 3.0).store(), two_thirds.next_down());
        assert_eq!((CHOP.load(-2.0) / 3.0).store(), -two_thirds.next_down());
    }

    #[test]
    fn truncating_operations_round_each_step_toward_zero() {
        let tenth = CHOP.load(1.0) / 10.0;
        assert_eq!(tenth.value, 0.1f64.next_down());
        assert_eq!((tenth * 10.0).value, 1.0f64.next_down());
        assert_eq!((NEAREST.load(1.0) / 10.0 * 10.0).value, 1.0);
        let tiny = f64::from(f32::EPSILON) * f64::EPSILON;
        let sum = CHOP.load(1.0)
            + X87 {
                value: tiny,
                fpu: CHOP,
            };
        assert_eq!(sum.value, 1.0);
        let difference = CHOP.load(1.0)
            - X87 {
                value: tiny,
                fpu: CHOP,
            };
        assert_eq!(difference.value, 1.0f64.next_down());
    }

    #[test]
    fn exact_results_are_unchanged_by_truncation() {
        assert_eq!((CHOP.load(0.25) * 3.0 + 0.5).store(), 1.25);
    }
}
