// JSON Pointer (RFC 6901) implementation for X07
// Provides navigation through parsed JSON documents using pointer syntax
//
// Pointer format: "/key1/key2/0" navigates to doc["key1"]["key2"][0]
// Escaped chars: ~0 = ~, ~1 = /

// Unescape a pointer segment: ~0 -> ~, ~1 -> /
fn _unescape_segment(seg: BytesView, start: i32, end: i32) -> Bytes {
    let len = end - start;
    let mut out = vec_u8_with_capacity(len);
    let mut i = start;
    for _ in start..end {
        if lt_u(i, end) {
            let c = view_get_u8(seg, i);
            if c == 126 {
                if lt_u(i + 1, end) {
                    let next = view_get_u8(seg, i + 1);
                    if next == 48 {
                        out = vec_u8_push(out, 126);
                        i = i + 2;
                    } else if next == 49 {
                        out = vec_u8_push(out, 47);
                        i = i + 2;
                    } else {
                        out = vec_u8_push(out, c);
                        i = i + 1;
                    }
                } else {
                    out = vec_u8_push(out, c);
                    i = i + 1;
                }
            } else {
                out = vec_u8_push(out, c);
                i = i + 1;
            }
        }
    }
    vec_u8_into_bytes(out)
}

// Parse an integer from a segment (for array indices)
fn _parse_index(seg: BytesView) -> i32 {
    let n = view_len(seg);
    if n == 0 {
        return 0 - 1;
    }
    // Check for leading zero (invalid except for "0")
    if n > 1 {
        if view_get_u8(seg, 0) == 48 {
            return 0 - 1;
        }
    }
    let mut val: i32 = 0;
    for i in 0..n {
        let c = view_get_u8(seg, i);
        if lt_u(c, 48) {
            return 0 - 1;
        }
        if ge_u(c, 58) {
            return 0 - 1;
        }
        val = val * 10 + (c - 48);
    }
    val
}

// Find the next '/' in a pointer string
fn _find_slash(ptr: BytesView, start: i32) -> i32 {
    let n = view_len(ptr);
    let mut i = start;
    let mut found = 0 - 1;
    for _ in start..n {
        if lt_u(i, n) {
            if found < 0 {
                if view_get_u8(ptr, i) == 47 {
                    found = i;
                }
                i = i + 1;
            }
        }
    }
    if found < 0 {
        n
    } else {
        found
    }
}

// Check if bytes match at a given offset
fn _bytes_eq(a: BytesView, b: BytesView) -> i32 {
    let a_len = view_len(a);
    let b_len = view_len(b);
    if a_len != b_len {
        return 0;
    }
    for i in 0..a_len {
        if view_get_u8(a, i) != view_get_u8(b, i) {
            return 0;
        }
    }
    1
}

// Skip whitespace in JSON
fn _skip_ws(json: BytesView, pos: i32) -> i32 {
    let n = view_len(json);
    let mut i = pos;
    for _ in pos..n {
        if lt_u(i, n) {
            let c = view_get_u8(json, i);
            if c == 32 || c == 9 || c == 10 || c == 13 {
                i = i + 1;
            }
        }
    }
    i
}

// Skip a JSON string, return position after closing quote
fn _skip_string(json: BytesView, pos: i32) -> i32 {
    let n = view_len(json);
    if ge_u(pos, n) {
        return pos;
    }
    if view_get_u8(json, pos) != 34 {
        return pos;
    }
    let mut i = pos + 1;
    for _ in (pos + 1)..n {
        if lt_u(i, n) {
            let c = view_get_u8(json, i);
            if c == 34 {
                return i + 1;
            }
            if c == 92 {
                i = i + 2;
            } else {
                i = i + 1;
            }
        }
    }
    n
}

// Extract string content (without quotes)
fn _extract_string(json: BytesView, pos: i32) -> Bytes {
    let n = view_len(json);
    if ge_u(pos, n) {
        return vec_u8_into_bytes(vec_u8_with_capacity(0));
    }
    if view_get_u8(json, pos) != 34 {
        return vec_u8_into_bytes(vec_u8_with_capacity(0));
    }
    let mut out = vec_u8_with_capacity(64);
    let mut i = pos + 1;
    for _ in (pos + 1)..n {
        if lt_u(i, n) {
            let c = view_get_u8(json, i);
            if c == 34 {
                return vec_u8_into_bytes(out);
            }
            if c == 92 {
                if lt_u(i + 1, n) {
                    let esc = view_get_u8(json, i + 1);
                    if esc == 110 {
                        out = vec_u8_push(out, 10);
                    } else if esc == 116 {
                        out = vec_u8_push(out, 9);
                    } else if esc == 114 {
                        out = vec_u8_push(out, 13);
                    } else if esc == 92 {
                        out = vec_u8_push(out, 92);
                    } else if esc == 34 {
                        out = vec_u8_push(out, 34);
                    } else if esc == 47 {
                        out = vec_u8_push(out, 47);
                    } else {
                        out = vec_u8_push(out, esc);
                    }
                    i = i + 2;
                } else {
                    i = i + 1;
                }
            } else {
                out = vec_u8_push(out, c);
                i = i + 1;
            }
        }
    }
    vec_u8_into_bytes(out)
}

// Skip a JSON value (any type)
fn _skip_value(json: BytesView, pos: i32) -> i32 {
    let n = view_len(json);
    let i = _skip_ws(json, pos);
    if ge_u(i, n) {
        return n;
    }
    let c = view_get_u8(json, i);
    if c == 34 {
        return _skip_string(json, i);
    }
    if c == 123 {
        // Object
        let mut j = i + 1;
        let mut depth = 1;
        for _ in (i + 1)..n {
            if lt_u(j, n) {
                if depth > 0 {
                    let ch = view_get_u8(json, j);
                    if ch == 123 {
                        depth = depth + 1;
                        j = j + 1;
                    } else if ch == 125 {
                        depth = depth - 1;
                        j = j + 1;
                    } else if ch == 34 {
                        j = _skip_string(json, j);
                    } else {
                        j = j + 1;
                    }
                }
            }
        }
        return j;
    }
    if c == 91 {
        // Array
        let mut j = i + 1;
        let mut depth = 1;
        for _ in (i + 1)..n {
            if lt_u(j, n) {
                if depth > 0 {
                    let ch = view_get_u8(json, j);
                    if ch == 91 {
                        depth = depth + 1;
                        j = j + 1;
                    } else if ch == 93 {
                        depth = depth - 1;
                        j = j + 1;
                    } else if ch == 34 {
                        j = _skip_string(json, j);
                    } else {
                        j = j + 1;
                    }
                }
            }
        }
        return j;
    }
    // Number, true, false, null
    let mut j = i;
    for _ in i..n {
        if lt_u(j, n) {
            let ch = view_get_u8(json, j);
            if ch == 44 || ch == 125 || ch == 93 || ch == 32 || ch == 9 || ch == 10 || ch == 13 {
                return j;
            }
            j = j + 1;
        }
    }
    n
}

// Get value at index in array, returns (start, end) packed as i32 pair
// Returns -1 for start if not found
fn _get_array_element(json: BytesView, arr_start: i32, index: i32) -> i32 {
    let n = view_len(json);
    let mut i = _skip_ws(json, arr_start + 1);
    if ge_u(i, n) {
        return 0 - 1;
    }
    if view_get_u8(json, i) == 93 {
        return 0 - 1;
    }
    let mut idx = 0;
    for _ in 0..n {
        if lt_u(i, n) {
            if idx == index {
                return i;
            }
            let end = _skip_value(json, i);
            i = _skip_ws(json, end);
            if ge_u(i, n) {
                return 0 - 1;
            }
            let c = view_get_u8(json, i);
            if c == 93 {
                return 0 - 1;
            }
            if c == 44 {
                i = _skip_ws(json, i + 1);
                idx = idx + 1;
            }
        }
    }
    0 - 1
}

// Get value for key in object, returns start position or -1
fn _get_object_member(json: BytesView, obj_start: i32, key: BytesView) -> i32 {
    let n = view_len(json);
    let mut i = _skip_ws(json, obj_start + 1);
    if ge_u(i, n) {
        return 0 - 1;
    }
    if view_get_u8(json, i) == 125 {
        return 0 - 1;
    }
    for _ in 0..n {
        if lt_u(i, n) {
            // Expect string key
            if view_get_u8(json, i) != 34 {
                return 0 - 1;
            }
            let key_str = _extract_string(json, i);
            let key_view = bytes_view(key_str);
            let key_match = _bytes_eq(key_view, key);
            i = _skip_string(json, i);
            i = _skip_ws(json, i);
            // Expect colon
            if ge_u(i, n) {
                return 0 - 1;
            }
            if view_get_u8(json, i) != 58 {
                return 0 - 1;
            }
            i = _skip_ws(json, i + 1);
            if key_match == 1 {
                return i;
            }
            // Skip value
            i = _skip_value(json, i);
            i = _skip_ws(json, i);
            if ge_u(i, n) {
                return 0 - 1;
            }
            let c = view_get_u8(json, i);
            if c == 125 {
                return 0 - 1;
            }
            if c == 44 {
                i = _skip_ws(json, i + 1);
            }
        }
    }
    0 - 1
}

/// Resolve a JSON Pointer against a JSON document
/// Returns the position in the JSON where the value starts, or -1 if not found
pub fn pointer_resolve(json: BytesView, ptr: BytesView) -> i32 {
    let ptr_len = view_len(ptr);
    let json_len = view_len(json);

    // Empty pointer = root
    if ptr_len == 0 {
        return _skip_ws(json, 0);
    }

    // Pointer must start with /
    if view_get_u8(ptr, 0) != 47 {
        return 0 - 1;
    }

    let mut pos = _skip_ws(json, 0);
    let mut seg_start = 1;

    for _ in 0..ptr_len {
        if lt_u(seg_start, ptr_len) {
            let seg_end = _find_slash(ptr, seg_start);
            let seg = _unescape_segment(ptr, seg_start, seg_end);
            let seg_view = bytes_view(seg);

            if ge_u(pos, json_len) {
                return 0 - 1;
            }

            let c = view_get_u8(json, pos);
            if c == 123 {
                // Object: use segment as key
                pos = _get_object_member(json, pos, seg_view);
                if pos < 0 {
                    return 0 - 1;
                }
            } else if c == 91 {
                // Array: parse segment as index
                let idx = _parse_index(seg_view);
                if idx < 0 {
                    return 0 - 1;
                }
                pos = _get_array_element(json, pos, idx);
                if pos < 0 {
                    return 0 - 1;
                }
            } else {
                // Not a container
                return 0 - 1;
            }

            seg_start = seg_end + 1;
        }
    }

    pos
}

/// Get the value at a JSON Pointer as bytes (extracts the raw JSON value)
pub fn pointer_get(json: BytesView, ptr: BytesView) -> Bytes {
    let pos = pointer_resolve(json, ptr);
    if pos < 0 {
        return vec_u8_into_bytes(vec_u8_with_capacity(0));
    }
    let end = _skip_value(json, pos);
    let len = end - pos;
    let mut out = vec_u8_with_capacity(len);
    for i in pos..end {
        out = vec_u8_push(out, view_get_u8(json, i));
    }
    vec_u8_into_bytes(out)
}

/// Get a string value at a JSON Pointer (extracts and unescapes string content)
pub fn pointer_get_string(json: BytesView, ptr: BytesView) -> Bytes {
    let pos = pointer_resolve(json, ptr);
    if pos < 0 {
        return vec_u8_into_bytes(vec_u8_with_capacity(0));
    }
    _extract_string(json, pos)
}

/// Check if a JSON Pointer exists in the document
pub fn pointer_exists(json: BytesView, ptr: BytesView) -> i32 {
    let pos = pointer_resolve(json, ptr);
    if pos < 0 {
        0
    } else {
        1
    }
}
