use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

use crate::tool::Tool;

static REGISTRY: OnceLock<Mutex<CapabilityRegistry>> = OnceLock::new();

pub fn registry() -> &'static Mutex<CapabilityRegistry> {
    REGISTRY.get_or_init(|| Mutex::new(CapabilityRegistry::new()))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CapabilityId([u8; 32]);

impl CapabilityId {
    pub fn of(name: &str) -> Self {
        let hash = blake3::hash(name.as_bytes());
        let mut id = [0u8; 32];
        id.copy_from_slice(hash.as_bytes());
        Self(id)
    }

    pub fn hex(&self) -> String {
        self.0.iter().fold(String::with_capacity(64), |mut s, b| {
            use std::fmt::Write;
            let _ = write!(s, "{b:02x}");
            s
        })
    }
}

pub trait Capability: Send + Sync {
    fn id(&self) -> CapabilityId;
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn tools(&self) -> Vec<Arc<dyn Tool>>;
}

pub struct CapabilityRegistry {
    entries: HashMap<CapabilityId, Vec<Arc<dyn Capability>>>,
    by_name: HashMap<String, CapabilityId>,
}

impl CapabilityRegistry {
    pub fn new() -> Self {
        Self { entries: HashMap::new(), by_name: HashMap::new() }
    }

    pub fn register(&mut self, cap: Arc<dyn Capability>) {
        let id = cap.id();
        let name = cap.name().to_string();
        self.by_name.insert(name, id);
        self.entries.entry(id).or_default().push(cap);
    }

    pub fn resolve(&self, id: &CapabilityId) -> Option<&Arc<dyn Capability>> {
        self.entries.get(id)?.first()
    }

    pub fn resolve_by_name(&self, name: &str) -> Option<&Arc<dyn Capability>> {
        self.by_name.get(name).and_then(|id| self.resolve(id))
    }

    pub fn list_names(&self) -> Vec<&str> {
        self.by_name.keys().map(|s| s.as_str()).collect()
    }
}

impl Default for CapabilityRegistry {
    fn default() -> Self { Self::new() }
}

pub fn install(cap: Arc<dyn Capability>) {
    registry()
        .lock()
        .expect("capability registry poisoned")
        .register(cap);
}

pub(crate) fn tools_for(names: &[String]) -> Vec<Arc<dyn Tool>> {
    let reg = registry().lock().expect("capability registry poisoned");
    let mut tools = Vec::new();
    for name in names {
        if let Some(cap) = reg.resolve_by_name(name) {
            tools.extend(cap.tools());
        }
    }
    tools
}

#[macro_export]
macro_rules! define_capability {
    ($vis:vis struct $name:ident {
        name: $cap_name:expr,
        desc: $cap_desc:expr,
        tools: [$($tool:ty),* $(,)?],
    }) => {
        $vis struct $name;

        impl $crate::kernel::Capability for $name {
            fn id(&self) -> $crate::kernel::CapabilityId {
                $crate::kernel::CapabilityId::of($cap_name)
            }
            fn name(&self) -> &str { $cap_name }
            fn description(&self) -> &str { $cap_desc }
            fn tools(&self) -> Vec<std::sync::Arc<dyn $crate::tool::Tool>> {
                vec![$(std::sync::Arc::new(<$tool>::default())),*]
            }
        }
    };

    ($vis:vis struct $name:ident {
        name: $cap_name:expr,
        desc: $cap_desc:expr,
        tools: [$($tool:ty),* $(,)?],
        on_event: $hook:expr,
    }) => {
        $crate::define_capability!($vis struct $name {
            name: $cap_name,
            desc: $cap_desc,
            tools: [$($tool),*],
        });

        // Hook variant: capability carries a lifecycle hook
        impl $name {
            pub fn hook(&self) -> &'static (dyn Fn(&str, &serde_json::Value) + Send + Sync) {
                &$hook
            }
        }
    };
}
