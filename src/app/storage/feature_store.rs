use super::sqlite::Database;

pub struct FeatureStore;

impl FeatureStore {
    pub fn track_usage(_db: &Database, _feature_id: &str) {
        // TODO: increment usage counter in feature_usage table
    }

    pub fn get_usage(_db: &Database, _feature_id: &str) -> u32 {
        0
    }
}
