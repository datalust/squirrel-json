use std::{borrow::Cow, fmt, str};

use super::{Offset, OffsetKind, Offsets, Slice};

use crate::{de::ActiveMapArr, unescape::unescape_trusted};

/**
A JSON document that's borrowed from an input buffer.

Documents can be constructed in one of two ways:

- By calling [`Document::scan_trusted`] to parse an input buffer on-demand.
- By calling [`Offsets::to_document_unchecked`] to use previously parsed offsets without re-parsing.
*/
#[derive(Clone)]
pub struct Document<'input> {
    pub(super) input: &'input [u8],
    pub(super) offsets: Offsets,
    pub(super) _detached_stack: Vec<ActiveMapArr>,
}

impl<'input> fmt::Debug for Document<'input> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        #[derive(Debug)]
        struct Map;

        #[derive(Debug)]
        struct Arr;

        #[derive(Debug)]
        struct Null;

        struct Offsets<'brw, 'input>(&'brw Document<'input>);

        impl<'brw, 'input> fmt::Debug for Offsets<'brw, 'input> {
            fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
                let mut list = f.debug_list();

                for (i, offset) in self.0.offsets.elements.iter().enumerate() {
                    match offset.kind {
                        OffsetKind::Str(s, escaped) => {
                            list.entry(&(
                                s.as_str(self.0.input),
                                escaped,
                                offset.position,
                                i,
                                offset.next,
                            ));
                        }
                        OffsetKind::Num(n) => {
                            list.entry(&(n.as_str(self.0.input), offset.position, i, offset.next));
                        }
                        OffsetKind::Map(any) => {
                            list.entry(&(Map, any, offset.position, i, offset.next));
                        }
                        OffsetKind::Arr(any) => {
                            list.entry(&(Arr, any, offset.position, i, offset.next));
                        }
                        OffsetKind::Bool(b) => {
                            list.entry(&(b, offset.position, i, offset.next));
                        }
                        OffsetKind::Null => {
                            list.entry(&(Null, offset.position, i, offset.next));
                        }
                    }
                }

                list.finish()
            }
        }

        f.debug_struct("Document")
            .field("input", &str::from_utf8(self.input))
            .field("err", &self.offsets.err)
            .field("offsets", &Offsets(self))
            .finish()
    }
}

/**
The kind of an element within a document.
*/
#[derive(Debug, Clone)]
pub enum Kind<'input, 'offsets> {
    Str(Str<'input>),
    Num(&'input str),
    Bool(bool),
    Null,
    Map(Map<'input, 'offsets>),
    Arr(Arr<'input, 'offsets>),
}

impl<'input, 'offsets> Kind<'input, 'offsets> {
    pub fn as_str(&self) -> Option<Str<'input>> {
        if let Kind::Str(s) = self {
            Some(*s)
        } else {
            None
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Str<'input>(&'input str, bool);

/**
A map within a document.
*/
#[derive(Debug, Clone)]
pub struct Map<'input, 'offsets> {
    input: &'input [u8],
    size_hint: u16,
    start_from_offset: Option<u16>,
    offsets: &'offsets Offsets,
}

/**
An array within a document.
*/
#[derive(Debug, Clone)]
pub struct Arr<'input, 'offsets> {
    input: &'input [u8],
    size_hint: u16,
    start_from_offset: Option<u16>,
    offsets: &'offsets Offsets,
}

impl<'input> Document<'input> {
    /**
    Treat the document like a map.
    */
    #[inline]
    pub fn as_map<'brw>(&'brw self) -> Map<'input, 'brw> {
        Map {
            input: self.input,
            size_hint: self.offsets.root_size_hint,
            start_from_offset: if self.offsets.root_size_hint > 0 {
                Some(0)
            } else {
                None
            },
            offsets: &self.offsets,
        }
    }
}

impl<'input> Str<'input> {
    /**
    Returns the underlying string, without attempting to unescape it.
    */
    #[inline]
    pub fn as_raw(&self) -> &str {
        self.0
    }

    /**
    Returns the underlying string.

    If the string is escaped then this method will allocate and unescape it.
    */
    #[inline]
    pub fn to_unescaped(&self) -> Cow<'input, str> {
        if self.1 {
            // SAFETY: The string to unescape was parsed from JSON
            // So it can't end with an unescaped `\`
            Cow::Owned(unsafe { unescape_trusted(self.0) })
        } else {
            Cow::Borrowed(self.0)
        }
    }
}

impl<'input, 'offsets> Map<'input, 'offsets> {
    /**
    The number of entries in the map, if known.
    */
    #[inline]
    pub fn size_hint(&self) -> usize {
        self.size_hint as usize
    }

    /**
    Iterate through entries in the map.
    */
    #[inline]
    pub fn entries<'brw>(
        &'brw self,
    ) -> impl Iterator<Item = (Str<'input>, Kind<'input, 'offsets>)> + 'brw {
        #[derive(Debug)]
        struct Entries<'brw, 'input, 'offsets> {
            inner: &'brw Map<'input, 'offsets>,
            key: Option<&'offsets Offset>,
            value: Option<(u16, &'offsets Offset)>,
        }

        impl<'brw, 'input, 'offsets> Iterator for Entries<'brw, 'input, 'offsets> {
            type Item = (Str<'input>, Kind<'input, 'offsets>);

            #[inline]
            fn next(&mut self) -> Option<Self::Item> {
                let key = self.key.take();
                let value = self.value.take();

                match (key, value) {
                    (Some(key), Some((value_offset, value))) => {
                        // the key must be a string
                        let entry_key = if let Some(key) = key.to_str(self.inner.input) {
                            key
                        } else {
                            return None;
                        };

                        let entry_value =
                            value.to_element(self.inner.input, self.inner.offsets, value_offset);

                        if let Some(next) = key.next {
                            self.key =
                                Some(get_unchecked!(self.inner.offsets.elements, next as usize));
                        }

                        if let Some(next) = value.next {
                            self.value = Some((
                                next,
                                get_unchecked!(self.inner.offsets.elements, next as usize),
                            ));
                        }

                        Some((entry_key, entry_value))
                    }
                    _ => None,
                }
            }
        }

        if let Some(first_part_offset) = self.start_from_offset {
            Entries {
                inner: self,
                key: Some(get_unchecked!(
                    self.offsets.elements,
                    first_part_offset as usize
                )),
                value: Some((
                    first_part_offset + 1,
                    get_unchecked!(self.offsets.elements, first_part_offset as usize + 1),
                )),
            }
        } else {
            Entries {
                inner: self,
                key: None,
                value: None,
            }
        }
    }
}

impl<'input, 'offsets> Arr<'input, 'offsets> {
    /**
    The number of elements in the array, if known.
    */
    #[inline]
    pub fn size_hint(&self) -> usize {
        self.size_hint as usize
    }

    /**
    Iterate through elements in the array.
    */
    #[inline]
    pub fn iter<'brw>(&'brw self) -> impl Iterator<Item = Kind<'input, 'offsets>> + 'brw {
        struct Iter<'brw, 'input, 'offsets> {
            inner: &'brw Arr<'input, 'offsets>,
            elem: Option<(u16, &'offsets Offset)>,
        }

        impl<'brw, 'input, 'offsets> Iterator for Iter<'brw, 'input, 'offsets> {
            type Item = Kind<'input, 'offsets>;

            #[inline]
            fn next(&mut self) -> Option<Self::Item> {
                let elem = self.elem.take();

                match elem {
                    Some((elem_offset, elem)) => {
                        let iter_elem =
                            elem.to_element(self.inner.input, self.inner.offsets, elem_offset);

                        if let Some(next) = elem.next {
                            self.elem = Some((
                                next,
                                get_unchecked!(self.inner.offsets.elements, next as usize),
                            ));
                        }

                        Some(iter_elem)
                    }
                    _ => None,
                }
            }
        }

        if let Some(first_part_offset) = self.start_from_offset {
            Iter {
                inner: self,
                elem: Some((
                    first_part_offset,
                    get_unchecked!(self.offsets.elements, first_part_offset as usize),
                )),
            }
        } else {
            Iter {
                inner: self,
                elem: None,
            }
        }
    }
}

impl Offset {
    #[inline]
    fn to_str<'input>(&self, input: &'input [u8]) -> Option<Str<'input>> {
        match self.kind {
            OffsetKind::Str(s, escaped) => Some(Str(s.as_str(input), escaped)),
            _ => None,
        }
    }

    #[inline]
    fn to_element<'input, 'offsets>(
        &self,
        input: &'input [u8],
        offsets: &'offsets Offsets,
        self_offset: u16,
    ) -> Kind<'input, 'offsets> {
        match self.kind {
            OffsetKind::Str(s, escaped) => Kind::Str(Str(s.as_str(input), escaped)),
            OffsetKind::Num(n) => Kind::Num(n.as_str(input)),
            OffsetKind::Map(len) => Kind::Map(Map {
                input,
                size_hint: len,
                start_from_offset: if len > 0 { Some(self_offset + 1) } else { None },
                offsets,
            }),
            OffsetKind::Arr(len) => Kind::Arr(Arr {
                input,
                size_hint: len,
                start_from_offset: if len > 0 { Some(self_offset + 1) } else { None },
                offsets,
            }),
            OffsetKind::Bool(b) => Kind::Bool(b),
            OffsetKind::Null => Kind::Null,
        }
    }
}

impl Slice {
    #[inline]
    fn as_str<'input>(&self, input: &'input [u8]) -> &'input str {
        from_utf8_unchecked!(offset_from_raw_parts!(
            input.as_ptr(),
            input.len(),
            self.offset as isize,
            self.len as usize
        ))
    }
}

#[cfg(any(test, feature = "serde_json"))]
impl<'input> Document<'input> {
    /**
    Convert a document into a [`serde_json::Value`].
    */
    pub fn to_value(&self) -> serde_json::Value {
        use std::str::FromStr;

        impl<'input, 'offsets> Kind<'input, 'offsets> {
            fn to_value(&self) -> serde_json::Value {
                match self {
                    Kind::Str(ref s) => serde_json::Value::String(s.to_unescaped().into_owned()),
                    Kind::Num(n) => match serde_json::Number::from_str(n.trim()) {
                        Ok(n) => serde_json::Value::Number(n),
                        _ => serde_json::Value::String((*n).to_owned()),
                    },
                    Kind::Bool(b) => serde_json::Value::Bool(*b),
                    Kind::Null => serde_json::Value::Null,
                    Kind::Map(ref map) => {
                        let mut value = serde_json::Map::with_capacity(map.size_hint());

                        for (k, v) in map.entries() {
                            value.insert(k.to_unescaped().into_owned(), v.to_value());
                        }

                        serde_json::Value::Object(value)
                    }
                    Kind::Arr(ref arr) => {
                        let mut value = Vec::with_capacity(arr.size_hint());

                        for e in arr.iter() {
                            value.push(e.to_value());
                        }

                        serde_json::Value::Array(value)
                    }
                }
            }
        }

        let doc = self.as_map();

        let mut map = serde_json::Map::with_capacity(doc.size_hint());

        for (k, v) in doc.entries() {
            map.insert(k.to_unescaped().into_owned(), v.to_value());
        }

        serde_json::Value::Object(map)
    }
}
