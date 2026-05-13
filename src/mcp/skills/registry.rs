use std::collections::HashMap;

use super::{Skill, SkillCategory, SkillToml};

pub struct SkillRegistry {
    skills: HashMap<String, Skill>,
}

impl SkillRegistry {
    pub fn new() -> Self {
        let mut skills = HashMap::new();

        let definitions: &[(&str, &str)] = &[
            ("explore_dataset", include_str!("definitions/explore_dataset.toml")),
            ("bootstrap_project", include_str!("definitions/bootstrap_project.toml")),
            ("setup_collection", include_str!("definitions/setup_collection.toml")),
            ("add_vector_search", include_str!("definitions/add_vector_search.toml")),
            ("debug_slow_query", include_str!("definitions/debug_slow_query.toml")),
            ("design_schema", include_str!("definitions/design_schema.toml")),
            ("build_search_pipeline", include_str!("definitions/build_search_pipeline.toml")),
            ("add_crud_endpoint", include_str!("definitions/add_crud_endpoint.toml")),
            ("find_anomalies", include_str!("definitions/find_anomalies.toml")),
            ("collection_health", include_str!("definitions/collection_health.toml")),
            ("optimize_performance", include_str!("definitions/optimize_performance.toml")),
            ("data_ingestion_pipeline", include_str!("definitions/data_ingestion_pipeline.toml")),
            ("migration_check", include_str!("definitions/migration_check.toml")),
        ];

        for (name, toml_str) in definitions {
            match toml::from_str::<SkillToml>(toml_str) {
                Ok(parsed) => {
                    let skill = Skill::from(parsed.skill);
                    skills.insert(name.to_string(), skill);
                }
                Err(e) => {
                    eprintln!("Failed to parse skill '{}': {}", name, e);
                }
            }
        }

        Self { skills }
    }

    pub fn list(&self, category: Option<SkillCategory>) -> Vec<&Skill> {
        let mut result: Vec<&Skill> = self.skills.values()
            .filter(|s| category.map_or(true, |c| s.category == c))
            .collect();
        result.sort_by(|a, b| a.name.cmp(&b.name));
        result
    }

    pub fn get(&self, name: &str) -> Option<&Skill> {
        self.skills.get(name)
    }

    pub fn len(&self) -> usize {
        self.skills.len()
    }
}

impl Default for SkillRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_registry_loads_all_skills() {
        let registry = SkillRegistry::new();
        assert_eq!(registry.len(), 13);
    }

    #[test]
    fn test_registry_get_by_name() {
        let registry = SkillRegistry::new();
        let skill = registry.get("explore_dataset").unwrap();
        assert_eq!(skill.name, "explore_dataset");
        assert_eq!(skill.category, SkillCategory::DataAnalysis);
        assert!(!skill.steps.is_empty());
        assert!(!skill.arguments.is_empty());
    }

    #[test]
    fn test_registry_filter_by_category() {
        let registry = SkillRegistry::new();
        let code_skills = registry.list(Some(SkillCategory::CodeScaffolding));
        assert_eq!(code_skills.len(), 3); // bootstrap_project + add_vector_search + add_crud_endpoint
        for skill in &code_skills {
            assert_eq!(skill.category, SkillCategory::CodeScaffolding);
        }
    }

    #[test]
    fn test_registry_list_all() {
        let registry = SkillRegistry::new();
        let all = registry.list(None);
        assert_eq!(all.len(), 13);
    }

    #[test]
    fn test_registry_get_nonexistent() {
        let registry = SkillRegistry::new();
        assert!(registry.get("nonexistent").is_none());
    }
}
