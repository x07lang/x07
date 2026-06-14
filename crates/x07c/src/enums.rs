//! RFC 0002 enums + match: lower `defenum` declarations to a tagged byte layout
//! and resolve enum variant-constructor call heads.
//!
//! An enum value is represented as brand-tagged `bytes` with layout
//! `[u32 tag][payload?]`: the tag is the variant's 0-based declaration index
//! written little-endian, optionally followed by a 4-byte little-endian payload
//! for a single-`i32`/`u32`-payload variant. Both fields reuse the
//! `codec.{write,read}_u32_le` codec (same as records), so the tag occupies a
//! 4-byte slot. The result is branded with the enum's fully-qualified name.
//! Payload types are validated to be `i32`/`u32` at parse time, so lowering is
//! infallible.

use crate::program::{EnumDef, EnumVariant};
use crate::types::Ty;
use crate::x07ast::AstEnumDef;

/// Size in bytes of the leading tag. The tag is written with
/// `codec.write_u32_le`, so it occupies a 4-byte little-endian slot; the
/// payload (when present) follows at this offset.
pub const TAG_SIZE: u32 = 4;

/// Size in bytes of a variant payload slot. Enums v1 only support a single
/// 32-bit scalar payload.
pub const PAYLOAD_SIZE: u32 = 4;

/// Lower parsed `defenum` declarations to their tagged byte layout.
pub fn lower_enums(ast: &[AstEnumDef]) -> Vec<EnumDef> {
    ast.iter()
        .map(|e| {
            let variants = e
                .variants
                .iter()
                .enumerate()
                .map(|(i, v)| EnumVariant {
                    name: v.name.clone(),
                    tag: i as u8,
                    payload: v.payload.as_ref().map(|_| Ty::I32),
                })
                .collect();
            EnumDef {
                name: e.name.clone(),
                variants,
            }
        })
        .collect()
}

/// A resolved enum operation for a call head of the form `<Enum>.<Variant>`.
///
/// Owned (the matched enum/variant are cloned) so callers can hold it across
/// `&mut self` calls without keeping the registry borrowed. Enums are few and
/// small, so cloning is cheap.
pub enum EnumOp {
    /// `<Enum>.<Variant>` constructor.
    Variant(EnumDef, EnumVariant),
}

/// Resolve a call head to an enum variant constructor, if `<Enum>` names a
/// declared enum and the suffix is one of its variants.
pub fn resolve_enum_op(enums: &[EnumDef], head: &str) -> Option<EnumOp> {
    for e in enums {
        let Some(rest) = head.strip_prefix(e.name.as_str()) else {
            continue;
        };
        let Some(suffix) = rest.strip_prefix('.') else {
            continue;
        };
        if let Some(variant) = e.variants.iter().find(|v| v.name == suffix) {
            return Some(EnumOp::Variant(e.clone(), variant.clone()));
        }
    }
    None
}

/// Find a declared enum by its fully-qualified brand name.
pub fn find_enum<'a>(enums: &'a [EnumDef], name: &str) -> Option<&'a EnumDef> {
    enums.iter().find(|e| e.name == name)
}
