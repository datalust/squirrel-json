use super::*;

// SAFETY: Callers must ensure `input` is valid UTF8
#[inline(always)]
pub(super) unsafe fn scan<'scan>(input: &'scan [u8], scan: &mut Scan, offsets: &mut Offsets) {
    let read_to = scan.input_len as isize;
    scan_block(ScanBlockInput {
        input,
        scan,
        offsets,
        read_to,
    });
}

// SAFETY: Callers must ensure `input` is valid UTF8
#[inline(always)]
#[cfg(not(wasm))]
pub(super) unsafe fn scan_to<'scan>(
    input: &'scan [u8],
    scan: &mut Scan,
    offsets: &mut Offsets,
    read_to: isize,
) {
    scan_block(ScanBlockInput {
        input,
        scan,
        offsets,
        read_to,
    });
}

#[inline(always)]
fn scan_block(i: ScanBlockInput) {
    'interest: while i.scan.input_offset < i.read_to {
        match i.scan.stack.active_map_arr.active_primitive.kind {
            ActivePrimitiveKind::None => {
                let curr_offset = i.scan.input_offset as usize;
                let curr = offset_deref_unchecked!(i.input, i.scan.input_offset);

                match_interest(ScanFnInput {
                    input: i.input,
                    scan: i.scan,
                    offsets: i.offsets,
                    curr_offset,
                    curr,
                });

                i.scan.input_offset += 1;
            }
            ActivePrimitiveKind::Str => {
                'str: while i.scan.input_offset < i.read_to {
                    let curr_offset = i.scan.input_offset as usize;
                    let curr = offset_deref_unchecked!(i.input, i.scan.input_offset);

                    match curr {
                        b'\\' => {
                            interest_escape(ScanFnInput {
                                input: i.input,
                                scan: i.scan,
                                offsets: i.offsets,
                                curr_offset,
                                curr,
                            });
                        }
                        b'"' => {
                            interest_str(ScanFnInput {
                                input: i.input,
                                scan: i.scan,
                                offsets: i.offsets,
                                curr_offset,
                                curr,
                            });

                            // if the string is finished then break the loop
                            // this will be falsy if the quote was escaped
                            if let ActivePrimitiveKind::None =
                                i.scan.stack.active_map_arr.active_primitive.kind
                            {
                                i.scan.input_offset += 1;
                                break 'str;
                            }
                        }
                        _ => (),
                    }

                    i.scan.input_offset += 1;
                }
            }
            ActivePrimitiveKind::Num => {
                'num: while i.scan.input_offset < i.read_to {
                    let curr_offset = i.scan.input_offset as usize;
                    let curr = offset_deref_unchecked!(i.input, i.scan.input_offset);

                    match curr {
                        b',' => {
                            interest_value_elem_end(ScanFnInput {
                                input: i.input,
                                scan: i.scan,
                                offsets: i.offsets,
                                curr_offset,
                                curr,
                            });

                            i.scan.input_offset += 1;
                            break 'num;
                        }
                        b'}' => {
                            interest_map_end(ScanFnInput {
                                input: i.input,
                                scan: i.scan,
                                offsets: i.offsets,
                                curr_offset,
                                curr,
                            });

                            i.scan.input_offset += 1;
                            break 'num;
                        }
                        b']' => {
                            interest_arr_end(ScanFnInput {
                                input: i.input,
                                scan: i.scan,
                                offsets: i.offsets,
                                curr_offset,
                                curr,
                            });

                            i.scan.input_offset += 1;
                            break 'num;
                        }
                        _ => (),
                    }

                    i.scan.input_offset += 1;
                }
            }
            ActivePrimitiveKind::Atom => {
                'atom: while i.scan.input_offset < i.read_to {
                    let curr_offset = i.scan.input_offset as usize;
                    let curr = offset_deref_unchecked!(i.input, i.scan.input_offset);

                    match curr {
                        b',' => {
                            interest_value_elem_end(ScanFnInput {
                                input: i.input,
                                scan: i.scan,
                                offsets: i.offsets,
                                curr_offset,
                                curr,
                            });

                            i.scan.input_offset += 1;
                            break 'atom;
                        }
                        b'}' => {
                            interest_map_end(ScanFnInput {
                                input: i.input,
                                scan: i.scan,
                                offsets: i.offsets,
                                curr_offset,
                                curr,
                            });

                            i.scan.input_offset += 1;
                            break 'atom;
                        }
                        b']' => {
                            interest_arr_end(ScanFnInput {
                                input: i.input,
                                scan: i.scan,
                                offsets: i.offsets,
                                curr_offset,
                                curr,
                            });

                            i.scan.input_offset += 1;
                            break 'atom;
                        }
                        _ => (),
                    }

                    i.scan.input_offset += 1;
                }
            }
        }
    }

    test_assert_eq!(i.read_to, i.scan.input_offset);
}

struct ScanBlockInput<'a, 'scan> {
    input: &'scan [u8],
    scan: &'a mut Scan,
    offsets: &'a mut Offsets,
    read_to: isize,
}
