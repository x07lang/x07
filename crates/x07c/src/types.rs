#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ty {
    I32,
    Bytes,
    BytesView,
    VecU8,
    OptionI32,
    OptionBytes,
    OptionBytesView,
    ResultI32,
    ResultBytes,
    ResultBytesView,
    ResultResultBytes,
    Iface,
    PtrConstU8,
    PtrMutU8,
    PtrConstVoid,
    PtrMutVoid,
    PtrConstI32,
    PtrMutI32,
    TaskScopeV1,
    BudgetScopeV1,
    TaskHandleBytesV1,
    TaskHandleResultBytesV1,
    TaskSlotV1,
    TaskSelectEvtV1,
    OptionTaskSelectEvtV1,
    Never,
}

impl Ty {
    pub fn parse_named(name: &str) -> Option<Self> {
        match name {
            "i32" => Some(Ty::I32),
            "bytes" => Some(Ty::Bytes),
            "bytes_view" => Some(Ty::BytesView),
            "vec_u8" => Some(Ty::VecU8),
            "option_i32" => Some(Ty::OptionI32),
            "option_bytes" => Some(Ty::OptionBytes),
            "option_bytes_view" => Some(Ty::OptionBytesView),
            "result_i32" => Some(Ty::ResultI32),
            "result_bytes" => Some(Ty::ResultBytes),
            "result_bytes_view" => Some(Ty::ResultBytesView),
            "result_result_bytes" => Some(Ty::ResultResultBytes),
            "iface" => Some(Ty::Iface),
            "ptr_const_u8" => Some(Ty::PtrConstU8),
            "ptr_mut_u8" => Some(Ty::PtrMutU8),
            "ptr_const_void" => Some(Ty::PtrConstVoid),
            "ptr_mut_void" => Some(Ty::PtrMutVoid),
            "ptr_const_i32" => Some(Ty::PtrConstI32),
            "ptr_mut_i32" => Some(Ty::PtrMutI32),
            _ => None,
        }
    }

    pub fn is_ffi_ty(self) -> bool {
        matches!(
            self,
            Ty::I32
                | Ty::PtrConstU8
                | Ty::PtrMutU8
                | Ty::PtrConstVoid
                | Ty::PtrMutVoid
                | Ty::PtrConstI32
                | Ty::PtrMutI32
        )
    }

    pub fn is_ptr_ty(self) -> bool {
        matches!(
            self,
            Ty::PtrConstU8
                | Ty::PtrMutU8
                | Ty::PtrConstVoid
                | Ty::PtrMutVoid
                | Ty::PtrConstI32
                | Ty::PtrMutI32
        )
    }
}
