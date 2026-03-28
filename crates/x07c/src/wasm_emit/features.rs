use std::collections::BTreeSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum WasmFeatureV1 {
    CoreFormsV1,
    ControlFlowV1,
    LiteralsV1,
    ViewReadV1,
    ViewToBytesV1,
    FmtBuiltinsV1,
    ParseBuiltinsV1,
    BytesBuiltinsV1,
    OpsArithV1,
    OpsCmpV1,
    OpsCmpSignedV1,
    OpsNeqV1,
    OpsBitwiseV1,
    OpsLogicV1,
    OpsShiftV1,
    CodecU32LeV1,
}

impl WasmFeatureV1 {
    pub const fn as_str(self) -> &'static str {
        match self {
            WasmFeatureV1::CoreFormsV1 => "CoreFormsV1",
            WasmFeatureV1::ControlFlowV1 => "ControlFlowV1",
            WasmFeatureV1::LiteralsV1 => "LiteralsV1",
            WasmFeatureV1::ViewReadV1 => "ViewReadV1",
            WasmFeatureV1::ViewToBytesV1 => "ViewToBytesV1",
            WasmFeatureV1::FmtBuiltinsV1 => "FmtBuiltinsV1",
            WasmFeatureV1::ParseBuiltinsV1 => "ParseBuiltinsV1",
            WasmFeatureV1::BytesBuiltinsV1 => "BytesBuiltinsV1",
            WasmFeatureV1::OpsArithV1 => "OpsArithV1",
            WasmFeatureV1::OpsCmpV1 => "OpsCmpV1",
            WasmFeatureV1::OpsCmpSignedV1 => "OpsCmpSignedV1",
            WasmFeatureV1::OpsNeqV1 => "OpsNeqV1",
            WasmFeatureV1::OpsBitwiseV1 => "OpsBitwiseV1",
            WasmFeatureV1::OpsLogicV1 => "OpsLogicV1",
            WasmFeatureV1::OpsShiftV1 => "OpsShiftV1",
            WasmFeatureV1::CodecU32LeV1 => "CodecU32LeV1",
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct WasmFeatureSetV1 {
    pub enabled: BTreeSet<WasmFeatureV1>,
}

impl WasmFeatureSetV1 {
    pub fn new(features: &[WasmFeatureV1]) -> Self {
        let mut s = BTreeSet::new();
        for f in features {
            s.insert(*f);
        }
        Self { enabled: s }
    }

    pub fn has(&self, f: WasmFeatureV1) -> bool {
        self.enabled.contains(&f)
    }
}

pub fn supported_features_v1() -> WasmFeatureSetV1 {
    WasmFeatureSetV1::new(&[
        WasmFeatureV1::CoreFormsV1,
        WasmFeatureV1::ControlFlowV1,
        WasmFeatureV1::LiteralsV1,
        WasmFeatureV1::ViewReadV1,
        WasmFeatureV1::ViewToBytesV1,
        WasmFeatureV1::FmtBuiltinsV1,
        WasmFeatureV1::ParseBuiltinsV1,
        WasmFeatureV1::BytesBuiltinsV1,
        WasmFeatureV1::OpsArithV1,
        WasmFeatureV1::OpsCmpV1,
        WasmFeatureV1::OpsCmpSignedV1,
        WasmFeatureV1::OpsNeqV1,
        WasmFeatureV1::OpsBitwiseV1,
        WasmFeatureV1::OpsLogicV1,
        WasmFeatureV1::OpsShiftV1,
        WasmFeatureV1::CodecU32LeV1,
    ])
}
