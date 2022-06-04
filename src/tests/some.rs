use rand::Rng;
use std::fmt::Write;

pub fn json_object() -> String {
    let mut s = String::new();
    let mut d = 0;

    write_object(&mut s, &mut d);

    s
}

fn write_any(s: &mut String, d: &mut usize) {
    if *d < 10 {
        match rng(6) {
            0 => write_object(s, d),
            1 => write_array(s, d),
            2 => write_bool(s),
            3 => write_number(s),
            4 => write_null(s),
            5 => write_string(s),
            _ => unreachable!(),
        }
    } else {
        match rng(4) {
            0 => write_bool(s),
            1 => write_number(s),
            2 => write_null(s),
            3 => write_string(s),
            _ => unreachable!(),
        }
    }
}

fn write_object(s: &mut String, d: &mut usize) {
    *d += 1;
    s.push('{');

    let mut first = true;
    for _ in 0..rng(10) {
        if !first {
            s.push(',');
        }
        first = false;

        write_string(s);
        s.push(':');
        write_any(s, d);
    }

    s.push('}');
    *d -= 1;
}

fn write_array(s: &mut String, d: &mut usize) {
    *d += 1;
    s.push('[');

    let mut first = true;
    for _ in 0..rng(10) {
        if !first {
            s.push(',');
        }
        first = false;

        write_any(s, d);
    }

    s.push(']');
    *d -= 1;
}

fn write_null(s: &mut String) {
    s.push_str("null");
}

fn write_bool(s: &mut String) {
    if rng_bool() {
        s.push_str("true");
    } else {
        s.push_str("false");
    }
}

fn write_string(s: &mut String) {
    s.push('"');

    for _ in 0..rng(10) {
        match rng(95) {
            0..=50 => {
                let i = rng(STR_1.len());
                s.push_str(&STR_1[i..i + 1]);
            }
            51..=60 => s.push_str(STR_2),
            61..=70 => s.push_str(STR_3),
            71..=80 => s.push_str(STR_4),
            81..=90 => s.push_str(STR_5),
            _ => s.push_str(&STR_0[0..rng(STR_0.len())]),
        }
    }

    s.push('"');
}

fn write_number(s: &mut String) {
    if rng_bool() {
        s.push('-');
    }

    match rng(3) {
        0 => write_integer(s),
        1 => write_decimal(s),
        2 => write_scientific(s),
        _ => unreachable!(),
    }
}

fn write_integer(s: &mut String) {
    write!(s, "{}", rng_u32()).unwrap();
}

fn write_decimal(s: &mut String) {
    // Keep precision low enough that floats can roundtrip
    write!(s, "{}.{}", rng_u32(), rng(300)).unwrap();
}

fn write_scientific(s: &mut String) {
    let e = match rng(4) {
        0 => "e",
        1 => "e-",
        2 => "E",
        3 => "E-",
        _ => unreachable!(),
    };

    // Try not to get too overboard with scientific numbers
    // They could easily overflow f64 or u64
    write!(s, "{}.{}{}{}", rng(10), rng(300), e, rng(7)).unwrap();
}

fn rng(to: usize) -> usize {
    rand::thread_rng().gen_range(0..to)
}

fn rng_bool() -> bool {
    rand::random()
}

fn rng_u32() -> u32 {
    rand::random()
}

// It's public domain, ok
const STR_0: &'static str =
    "Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris nisi ut aliquip ex ea commodo consequat. Duis aute irure dolor in reprehenderit in voluptate velit esse cillum dolore eu fugiat nulla pariatur. Excepteur sint occaecat cupidatat non proident, sunt in culpa qui officia deserunt mollit anim id est laborum.";

const STR_1: &'static str = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";

const STR_2: &'static str = "\\\"";

const STR_3: &'static str = "\\u58c1";

const STR_4: &'static str = "壁";

const STR_5: &'static str = "😄";
