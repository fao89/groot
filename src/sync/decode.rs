const PAD: u8 = 61u8;

pub fn a2b_base64(b: Vec<u8>, strict_mode: bool) -> Result<Vec<u8>, base64::DecodeError> {
    #[rustfmt::skip]
        // Converts between ASCII and base-64 characters. The index of a given number yields the
        // number in ASCII while the value of said index yields the number in base-64. For example
        // "=" is 61 in ASCII but 0 (since it's the pad character) in base-64, so BASE64_TABLE[61] == 0
        const BASE64_TABLE: [i8; 256] = [
            -1,-1,-1,-1, -1,-1,-1,-1, -1,-1,-1,-1, -1,-1,-1,-1,
            -1,-1,-1,-1, -1,-1,-1,-1, -1,-1,-1,-1, -1,-1,-1,-1,
            -1,-1,-1,-1, -1,-1,-1,-1, -1,-1,-1,62, -1,-1,-1,63,
            52,53,54,55, 56,57,58,59, 60,61,-1,-1, -1, 0,-1,-1, /* Note PAD->0 */
            -1, 0, 1, 2,  3, 4, 5, 6,  7, 8, 9,10, 11,12,13,14,
            15,16,17,18, 19,20,21,22, 23,24,25,-1, -1,-1,-1,-1,
            -1,26,27,28, 29,30,31,32, 33,34,35,36, 37,38,39,40,
            41,42,43,44, 45,46,47,48, 49,50,51,-1, -1,-1,-1,-1,

            -1,-1,-1,-1, -1,-1,-1,-1, -1,-1,-1,-1, -1,-1,-1,-1,
            -1,-1,-1,-1, -1,-1,-1,-1, -1,-1,-1,-1, -1,-1,-1,-1,
            -1,-1,-1,-1, -1,-1,-1,-1, -1,-1,-1,-1, -1,-1,-1,-1,
            -1,-1,-1,-1, -1,-1,-1,-1, -1,-1,-1,-1, -1,-1,-1,-1,
            -1,-1,-1,-1, -1,-1,-1,-1, -1,-1,-1,-1, -1,-1,-1,-1,
            -1,-1,-1,-1, -1,-1,-1,-1, -1,-1,-1,-1, -1,-1,-1,-1,
            -1,-1,-1,-1, -1,-1,-1,-1, -1,-1,-1,-1, -1,-1,-1,-1,
            -1,-1,-1,-1, -1,-1,-1,-1, -1,-1,-1,-1, -1,-1,-1,-1,
        ];

    if b.is_empty() {
        return Ok(vec![]);
    }

    if strict_mode && b[0] == PAD {
        return Err(base64::DecodeError::InvalidByte(0, 61));
    }

    let mut decoded: Vec<u8> = vec![];

    let mut quad_pos = 0; // position in the nibble
    let mut pads = 0;
    let mut left_char: u8 = 0;
    let mut padding_started = false;
    for (i, &el) in b.iter().enumerate() {
        if el == PAD {
            padding_started = true;

            pads += 1;
            if quad_pos >= 2 && quad_pos + pads >= 4 {
                if strict_mode && i + 1 < b.len() {
                    // Represents excess data after padding error
                    return Err(base64::DecodeError::InvalidLastSymbol(i, PAD));
                }

                return Ok(decoded);
            }

            continue;
        }

        let binary_char = BASE64_TABLE[el as usize];
        if binary_char >= 64 || binary_char == -1 {
            if strict_mode {
                // Represents non-base64 data error
                return Err(base64::DecodeError::InvalidByte(i, el));
            }
            continue;
        }

        if strict_mode && padding_started {
            // Represents discontinuous padding error
            return Err(base64::DecodeError::InvalidByte(i, PAD));
        }
        pads = 0;

        // Decode individual ASCII character
        match quad_pos {
            0 => {
                quad_pos = 1;
                left_char = binary_char as u8;
            }
            1 => {
                quad_pos = 2;
                decoded.push((left_char << 2) | (binary_char >> 4) as u8);
                left_char = (binary_char & 0x0f) as u8;
            }
            2 => {
                quad_pos = 3;
                decoded.push((left_char << 4) | (binary_char >> 2) as u8);
                left_char = (binary_char & 0x03) as u8;
            }
            3 => {
                quad_pos = 0;
                decoded.push((left_char << 6) | binary_char as u8);
                left_char = 0;
            }
            _ => unsafe {
                // quad_pos is only assigned in this match statement to constants
                std::hint::unreachable_unchecked()
            },
        }
    }

    match quad_pos {
        0 => Ok(decoded),
        1 => Err(base64::DecodeError::InvalidLastSymbol(
            decoded.len() / 3 * 4 + 1,
            0,
        )),
        _ => Err(base64::DecodeError::InvalidLength),
    }
}
