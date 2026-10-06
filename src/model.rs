#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecutionPlane {
    Light,
    Servo,
    Continuity,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Confidence {
    ProvenBounded,
    Complex,
    Uncertain,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Signal {
    Script,
    ModuleScript,
    CustomElement,
    ShadowDom,
    WebAssembly,
    WebRtc,
    WebGl,
    Canvas,
    ServiceWorker,
    DynamicImport,
    NestedBrowsingContext,
    EmbeddedObject,
    Form,
    Media,
    StyleSheet,
    UnsupportedLightSyntax,
    UnknownActiveContent,
}

#[derive(Debug, Clone)]
pub struct Classification {
    pub plane: ExecutionPlane,
    pub confidence: Confidence,
    pub signals: Vec<Signal>,
    pub rationale: String,
}

#[derive(Debug, Clone, Copy)]
pub struct ResourceState {
    pub memory_available_ratio: f32,
    pub cpu_busy_ratio: f32,
    pub visible: bool,
    pub recently_interacted: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LifecycleState {
    Hot,
    Warm,
    Cold,
    Frozen,
}
