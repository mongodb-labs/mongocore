pub mod detect;
pub mod model_gen;
pub mod query_gen;
pub mod index_gen;
pub mod templates;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Language {
    Python,
    TypeScript,
    Go,
    Java,
}

impl Language {
    pub fn extension(&self) -> &str {
        match self {
            Self::Python => "py",
            Self::TypeScript => "ts",
            Self::Go => "go",
            Self::Java => "java",
        }
    }

    pub fn display_name(&self) -> &str {
        match self {
            Self::Python => "python",
            Self::TypeScript => "typescript",
            Self::Go => "go",
            Self::Java => "java",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Framework {
    FastApi,
    Django,
    Flask,
    Express,
    NextJs,
    SpringBoot,
    Gin,
    Chi,
    None,
}

impl Framework {
    pub fn display_name(&self) -> &str {
        match self {
            Self::FastApi => "FastAPI",
            Self::Django => "Django",
            Self::Flask => "Flask",
            Self::Express => "Express",
            Self::NextJs => "Next.js",
            Self::SpringBoot => "Spring Boot",
            Self::Gin => "Gin",
            Self::Chi => "Chi",
            Self::None => "none",
        }
    }

    pub fn skill_recommendation(&self) -> Option<&str> {
        match self {
            Self::FastApi => Some("Combine with a FastAPI skill to generate complete route handlers with request validation and OpenAPI docs."),
            Self::Django => Some("Combine with a Django skill to generate views, serializers, and URL routing."),
            Self::Express => Some("Combine with an Express skill to generate route handlers with middleware and error handling."),
            Self::NextJs => Some("Combine with a Next.js skill to generate server actions or API route handlers."),
            Self::SpringBoot => Some("Combine with a Spring Boot skill to generate @RestController endpoints with dependency injection."),
            Self::Gin => Some("Combine with a Gin skill to generate handler functions with proper context propagation."),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct DetectedStack {
    pub language: Language,
    pub framework: Framework,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_language_extension() {
        assert_eq!(Language::Python.extension(), "py");
        assert_eq!(Language::TypeScript.extension(), "ts");
        assert_eq!(Language::Go.extension(), "go");
        assert_eq!(Language::Java.extension(), "java");
    }

    #[test]
    fn test_framework_recommendation() {
        assert!(Framework::FastApi.skill_recommendation().is_some());
        assert!(Framework::None.skill_recommendation().is_none());
    }
}
