//! RFC 0002 records: lower `defrecord` declarations to a fixed byte layout and
//! resolve record constructor / accessor call heads.
//!
//! A record value is represented as brand-tagged `bytes`: the constructor packs
//! its fields little-endian into a fixed-size buffer and brands the result with
//! the record's fully-qualified name; accessors read a field at its offset from
//! a value carrying that brand. Field types are validated to be `i32`/`u32` at
//! parse time, so lowering is infallible.

use crate::program::{RecordDef, RecordField};
use crate::types::Ty;
use crate::x07ast::AstRecordDef;

/// Size in bytes of a record field's fixed slot. Records v1 only support
/// 32-bit scalar fields.
pub const FIELD_SIZE: u32 = 4;

/// Lower parsed `defrecord` declarations to their fixed byte layout.
pub fn lower_records(ast: &[AstRecordDef]) -> Vec<RecordDef> {
    ast.iter()
        .map(|r| {
            let mut fields = Vec::with_capacity(r.fields.len());
            let mut offset = 0u32;
            for f in &r.fields {
                fields.push(RecordField {
                    name: f.name.clone(),
                    ty: Ty::I32,
                    offset,
                });
                offset += FIELD_SIZE;
            }
            RecordDef {
                name: r.name.clone(),
                fields,
                size: offset,
            }
        })
        .collect()
}

/// A resolved record operation for a call head of the form `<Record>.<op>`.
///
/// Owned (the matched record/field are cloned) so callers can hold it across
/// `&mut self` calls without keeping the registry borrowed. Records are few and
/// small, so cloning is cheap.
pub enum RecordOp {
    /// `<Record>.make` constructor.
    Make(RecordDef),
    /// `<Record>.<field>` accessor.
    Field(RecordDef, RecordField),
}

/// Resolve a call head to a record constructor/accessor, if `<Record>` names a
/// declared record and the suffix is `make` or one of its fields.
pub fn resolve_record_op(records: &[RecordDef], head: &str) -> Option<RecordOp> {
    for r in records {
        let Some(rest) = head.strip_prefix(r.name.as_str()) else {
            continue;
        };
        let Some(suffix) = rest.strip_prefix('.') else {
            continue;
        };
        if suffix == "make" {
            return Some(RecordOp::Make(r.clone()));
        }
        if let Some(field) = r.fields.iter().find(|f| f.name == suffix) {
            return Some(RecordOp::Field(r.clone(), field.clone()));
        }
    }
    None
}
