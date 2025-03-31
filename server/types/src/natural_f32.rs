use errors::validation::ValidationError;
use errors::validation::ValidationError::NegativeValue;
#[cfg(feature = "arbitrary")]
use proptest_derive::Arbitrary;
use std::cmp::Ordering;
use std::iter::Sum;
use std::ops::{Add, Div};

/// f32 の範囲で非負数を表す型
#[cfg_attr(feature = "arbitrary", derive(Arbitrary))]
#[derive(Clone, Copy, Debug)]
pub struct NonNegativeF32(f32);

impl NonNegativeF32 {
    pub fn try_new(value: f32) -> Result<Self, ValidationError> {
        if value < 0.0 {
            return Err(NegativeValue);
        }

        Ok(Self(value))
    }

    /// "バリデーションを行わずに" [`NonNegativeF32`] を生成します。
    ///
    /// # Safety
    /// [`value`] が負の値を持たないことを確実に保証している場合にのみ使用してください。
    pub const unsafe fn new_unchecked(value: f32) -> Self {
        Self(value)
    }

    pub const fn into_inner(self) -> f32 {
        self.0
    }
}

impl Add for NonNegativeF32 {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        NonNegativeF32(self.0 + rhs.0)
    }
}

impl Sum for NonNegativeF32 {
    fn sum<I: Iterator<Item = Self>>(iter: I) -> Self {
        iter.fold(NonNegativeF32(0.0), Add::add)
    }
}

impl Div for NonNegativeF32 {
    type Output = Self;

    fn div(self, rhs: Self) -> Self::Output {
        if rhs.0 == 0.0 {
            panic!("Division by zero");
        }

        NonNegativeF32(self.0 / rhs.0)
    }
}

impl PartialEq<f32> for NonNegativeF32 {
    fn eq(&self, other: &f32) -> bool {
        self.0 == *other
    }
}

impl PartialOrd<f32> for NonNegativeF32 {
    fn partial_cmp(&self, other: &f32) -> Option<Ordering> {
        if self.0 < *other {
            Some(Ordering::Less)
        } else if self.0 > *other {
            Some(Ordering::Greater)
        } else {
            Some(Ordering::Equal)
        }
    }
}

#[cfg(feature = "proptest")]
#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    proptest! {
        #[test]
        fn non_negative_f32(value: f32) {
            let result = NonNegativeF32::try_new(value);

            prop_assert_eq!(result.is_ok(), value >= 0.0);
        }
    }
}
