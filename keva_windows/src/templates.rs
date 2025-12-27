//! HTML templates for WebView content.

use windows_strings::PCWSTR;

/// w! macro that works on &str
macro_rules! w_from_bytes {
    ($s:expr) => {{
        const OUTPUT_LEN: usize = windows_strings::utf16_len($s) + 1;
        const OUTPUT: &[u16; OUTPUT_LEN] = {
            let mut buffer = [0; OUTPUT_LEN];
            let mut input_pos = 0;
            let mut output_pos = 0;
            while let Some((mut code_point, new_pos)) =
                windows_strings::decode_utf8_char($s, input_pos)
            {
                input_pos = new_pos;
                if code_point <= 0xffff {
                    buffer[output_pos] = code_point as u16;
                    output_pos += 1;
                } else {
                    code_point -= 0x10000;
                    buffer[output_pos] = 0xd800 + (code_point >> 10) as u16;
                    output_pos += 1;
                    buffer[output_pos] = 0xdc00 + (code_point & 0x3ff) as u16;
                    output_pos += 1;
                }
            }
            &{ buffer }
        };
        windows_strings::PCWSTR::from_raw(OUTPUT.as_ptr())
    }};
}

pub const APP_HTML: &[u8] = include_bytes!("ui_html/app.html");
pub const APP_HTML_W: PCWSTR = w_from_bytes!(APP_HTML);
