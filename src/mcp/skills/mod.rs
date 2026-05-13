pub mod registry;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Skill {
    pub name: String,
    pub description: String,
    pub category: SkillCategory,
    pub arguments: Vec<SkillArgument>,
    pub steps: Vec<SkillStep>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SkillCategory {
    DatabaseWorkflows,
    CodeScaffolding,
    DataAnalysis,
    Operations,
}

impl SkillCategory {
    pub fn display_name(&self) -> &str {
        match self {
            Self::DatabaseWorkflows => "Database Workflows",
            Self::CodeScaffolding => "Code Scaffolding",
            Self::DataAnalysis => "Data Analysis",
            Self::Operations => "Operations",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillArgument {
    pub name: String,
    pub description: String,
    pub required: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillStep {
    pub description: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool: Option<String>,
    #[serde(default)]
    pub dynamic: bool,
    #[serde(default)]
    pub analysis: bool,
    #[serde(default)]
    pub synthesis: bool,
}

/// TOML file structure for skill definitions.
#[derive(Debug, Deserialize)]
pub struct SkillToml {
    pub skill: SkillTomlInner,
}

#[derive(Debug, Deserialize)]
pub struct SkillTomlInner {
    pub name: String,
    pub description: String,
    pub category: SkillCategory,
    #[serde(default)]
    pub arguments: Vec<SkillArgument>,
    #[serde(default)]
    pub steps: Vec<SkillStep>,
}

impl From<SkillTomlInner> for Skill {
    fn from(inner: SkillTomlInner) -> Self {
        Self {
            name: inner.name,
            description: inner.description,
            category: inner.category,
            arguments: inner.arguments,
            steps: inner.steps,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_skill_toml() {
        let toml_str = r#"
[skill]
name = "explore_dataset"
description = "Explore a database"
category = "data_analysis"

[[skill.arguments]]
name = "database"
description = "Database to explore"
required = true

[[skill.steps]]
description = "List collections"
tool = "list_collections"

[[skill.steps]]
description = "Analyze results"
analysis = true
"#;
        let parsed: SkillToml = toml::from_str(toml_str).unwrap();
        let skill = Skill::from(parsed.skill);
        assert_eq!(skill.name, "explore_dataset");
        assert_eq!(skill.category, SkillCategory::DataAnalysis);
        assert_eq!(skill.arguments.len(), 1);
        assert!(skill.arguments[0].required);
        assert_eq!(skill.steps.len(), 2);
        assert_eq!(skill.steps[0].tool.as_deref(), Some("list_collections"));
        assert!(skill.steps[1].analysis);
    }

    #[test]
    fn test_skill_category_display() {
        assert_eq!(SkillCategory::DatabaseWorkflows.display_name(), "Database Workflows");
        assert_eq!(SkillCategory::CodeScaffolding.display_name(), "Code Scaffolding");
        assert_eq!(SkillCategory::DataAnalysis.display_name(), "Data Analysis");
        assert_eq!(SkillCategory::Operations.display_name(), "Operations");
    }
}
