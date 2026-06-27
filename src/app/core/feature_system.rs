use std::collections::HashMap;

#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct Feature {
    pub id: String,
    pub usage: u32,
    pub pinned: bool,
    pub mode_scope: String,
}

pub struct FeatureSystem {
    features: HashMap<String, Feature>,
}

impl FeatureSystem {
    pub fn new() -> Self {
        Self {
            features: HashMap::new(),
        }
    }

    pub fn register(&mut self, id: &str, mode_scope: &str) {
        self.features.entry(id.to_string()).or_insert(Feature {
            id: id.to_string(),
            usage: 0,
            pinned: false,
            mode_scope: mode_scope.to_string(),
        });
    }

    pub fn use_feature(&mut self, id: &str) {
        if let Some(f) = self.features.get_mut(id) {
            f.usage = f.usage.saturating_add(1);
        }
    }

    pub fn toggle_pin(&mut self, id: &str) {
        if let Some(f) = self.features.get_mut(id) {
            f.pinned = !f.pinned;
        }
    }

    pub fn visible_features(&self, mode: &str) -> Vec<&Feature> {
        let mut list: Vec<_> = self.features.values()
            .filter(|f| f.mode_scope == mode)
            .collect();
        list.sort_by(|a, b| b.usage.cmp(&a.usage));
        list
    }

    pub fn pinned_features(&self, mode: &str) -> Vec<&Feature> {
        self.features.values()
            .filter(|f| f.pinned && f.mode_scope == mode)
            .collect()
    }
}
