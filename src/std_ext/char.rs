use std::{convert::TryFrom, error::Error, fmt};

#[derive(Debug, Clone, PartialEq)]
pub struct CharTryFromSurrogateError {}

impl Error for CharTryFromSurrogateError {}

impl fmt::Display for CharTryFromSurrogateError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        "converted integer out of range for `char`".fmt(f)
    }
}

pub fn try_from_utf16_surrogate_pair(
    high: u16,
    low: u16,
) -> Result<char, CharTryFromSurrogateError> {
    // Courtesy of: http://www.russellcottrell.com/greek/utilities/SurrogatePairCalculator.htm
    let try_from = || {
        Some(
            (((high as u32).checked_sub(0xD800)?) * 0x400)
                + ((low as u32).checked_sub(0xDC00)?)
                + 0x10000,
        )
    };

    let code = try_from().ok_or(CharTryFromSurrogateError {})?;
    char::try_from(code).map_err(|_| CharTryFromSurrogateError {})
}
