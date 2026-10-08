//! Values written on the right-hand side of a cfg constant.
//!
//! [`ConfigValue`] is deliberately small and owned by this crate. It records
//! exactly what TLC's `ModelConfig.parseValue` builds, in source order, so it
//! can be lowered to `dive-value` once that crate exists. Lowering must:
//!
//! - turn [`ConfigValue::Set`] into a normalized set, so duplicates collapse
//!   (TLC builds an unnormalized `SetEnumValue` and normalizes it later);
//! - intern [`ConfigValue::ModelValue`] names in the order of
//!   [`ModelConfig::model_values`](crate::ModelConfig::model_values), which
//!   is the creation order TLC uses for `ModelValue` indices;
//! - keep strings verbatim: TLC does not process escapes in cfg strings.

use dive_syntax::TextRange;

/// A value with its source span.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Spanned<T> {
    /// The value.
    pub value: T,
    /// Its bytes in the cfg file.
    pub span: TextRange,
}

/// A constant value as TLC's cfg reader parses it.
///
/// The grammar is TLC's, which is narrower than TLA+'s: integers are
/// non-negative 32-bit decimals, there are no tuples, records or functions,
/// and any other single token becomes a model value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfigValue {
    /// A decimal integer literal in `0..=2^31-1`. Negative literals do not
    /// exist: in `N = -1`, `-` is a model value.
    Int(i32),
    /// A string literal's contents between the quotes, verbatim. TLC does
    /// not process escapes here, so `"a\nb"` is four characters.
    String(String),
    /// `TRUE` or `FALSE`.
    Bool(bool),
    /// `{v1, ..., vn}`, elements in source order, duplicates kept.
    Set(Vec<Spanned<ConfigValue>>),
    /// Any other token: a model value with that name. Usually an
    /// identifier, but TLC accepts any token here (`-`, `<<`, `}`).
    ModelValue(String),
}

impl ConfigValue {
    /// The model value's type tag, if this is a typed model value.
    ///
    /// TLC treats a model value whose name is longer than two characters
    /// and whose second character is `_` as typed by its first character
    /// (`ModelValue` constructor): `a_1` has type `a`.
    #[must_use]
    pub fn model_value_type(&self) -> Option<char> {
        let Self::ModelValue(name) = self else {
            return None;
        };
        // TLC counts UTF-16 code units.
        let units: Vec<u16> = name.encode_utf16().take(3).collect();
        if units.len() > 2 && units.get(1) == Some(&u16::from(b'_')) {
            name.chars().next()
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn typed_model_values() {
        let mv = |s: &str| ConfigValue::ModelValue(s.to_owned());
        assert_eq!(mv("a_1").model_value_type(), Some('a'));
        assert_eq!(mv("a_").model_value_type(), None);
        assert_eq!(mv("ab").model_value_type(), None);
        assert_eq!(mv("_a").model_value_type(), None);
        assert_eq!(ConfigValue::Int(1).model_value_type(), None);
    }
}
