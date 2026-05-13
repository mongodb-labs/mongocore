use std::path::Path;

use super::{DetectedStack, Framework, Language};

pub fn detect_stack(workspace_root: &Path) -> Option<DetectedStack> {
    let language = detect_language(workspace_root)?;
    let framework = detect_framework(workspace_root, language);
    Some(DetectedStack { language, framework })
}

fn detect_language(root: &Path) -> Option<Language> {
    if root.join("pyproject.toml").exists()
        || root.join("requirements.txt").exists()
        || root.join("setup.py").exists()
    {
        return Some(Language::Python);
    }
    if root.join("package.json").exists() || root.join("tsconfig.json").exists() {
        return Some(Language::TypeScript);
    }
    if root.join("go.mod").exists() {
        return Some(Language::Go);
    }
    if root.join("pom.xml").exists()
        || root.join("build.gradle").exists()
        || root.join("build.gradle.kts").exists()
    {
        return Some(Language::Java);
    }
    None
}

fn detect_framework(root: &Path, language: Language) -> Framework {
    match language {
        Language::Python => detect_python_framework(root),
        Language::TypeScript => detect_typescript_framework(root),
        Language::Go => detect_go_framework(root),
        Language::Java => detect_java_framework(root),
    }
}

fn detect_python_framework(root: &Path) -> Framework {
    let files = ["pyproject.toml", "requirements.txt", "setup.py"];
    for file in &files {
        let path = root.join(file);
        if let Ok(content) = std::fs::read_to_string(&path) {
            let lower = content.to_lowercase();
            if lower.contains("fastapi") {
                return Framework::FastApi;
            }
            if lower.contains("django") {
                return Framework::Django;
            }
            if lower.contains("flask") {
                return Framework::Flask;
            }
        }
    }
    Framework::None
}

fn detect_typescript_framework(root: &Path) -> Framework {
    let path = root.join("package.json");
    if let Ok(content) = std::fs::read_to_string(&path) {
        let lower = content.to_lowercase();
        if lower.contains("\"next\"") || lower.contains("\"next\":") {
            return Framework::NextJs;
        }
        if lower.contains("\"express\"") || lower.contains("\"express\":") {
            return Framework::Express;
        }
    }
    Framework::None
}

fn detect_go_framework(root: &Path) -> Framework {
    let path = root.join("go.mod");
    if let Ok(content) = std::fs::read_to_string(&path) {
        if content.contains("github.com/gin-gonic/gin") {
            return Framework::Gin;
        }
        if content.contains("github.com/go-chi/chi") {
            return Framework::Chi;
        }
    }
    Framework::None
}

fn detect_java_framework(root: &Path) -> Framework {
    let files = ["pom.xml", "build.gradle", "build.gradle.kts"];
    for file in &files {
        let path = root.join(file);
        if let Ok(content) = std::fs::read_to_string(&path) {
            if content.contains("spring-boot") || content.contains("org.springframework.boot") {
                return Framework::SpringBoot;
            }
        }
    }
    Framework::None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_detect_python_from_pyproject() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("pyproject.toml"), "[project]\nname = \"myapp\"").unwrap();
        let stack = detect_stack(dir.path()).unwrap();
        assert_eq!(stack.language, Language::Python);
        assert_eq!(stack.framework, Framework::None);
    }

    #[test]
    fn test_detect_fastapi() {
        let dir = TempDir::new().unwrap();
        fs::write(
            dir.path().join("requirements.txt"),
            "fastapi>=0.100\nuvicorn",
        )
        .unwrap();
        let stack = detect_stack(dir.path()).unwrap();
        assert_eq!(stack.language, Language::Python);
        assert_eq!(stack.framework, Framework::FastApi);
    }

    #[test]
    fn test_detect_typescript_express() {
        let dir = TempDir::new().unwrap();
        fs::write(
            dir.path().join("package.json"),
            r#"{"dependencies":{"express":"^4.18"}}"#,
        )
        .unwrap();
        let stack = detect_stack(dir.path()).unwrap();
        assert_eq!(stack.language, Language::TypeScript);
        assert_eq!(stack.framework, Framework::Express);
    }

    #[test]
    fn test_detect_nextjs() {
        let dir = TempDir::new().unwrap();
        fs::write(
            dir.path().join("package.json"),
            r#"{"dependencies":{"next":"14.0","react":"18"}}"#,
        )
        .unwrap();
        let stack = detect_stack(dir.path()).unwrap();
        assert_eq!(stack.language, Language::TypeScript);
        assert_eq!(stack.framework, Framework::NextJs);
    }

    #[test]
    fn test_detect_go_gin() {
        let dir = TempDir::new().unwrap();
        fs::write(
            dir.path().join("go.mod"),
            "module myapp\nrequire github.com/gin-gonic/gin v1.9",
        )
        .unwrap();
        let stack = detect_stack(dir.path()).unwrap();
        assert_eq!(stack.language, Language::Go);
        assert_eq!(stack.framework, Framework::Gin);
    }

    #[test]
    fn test_detect_java_spring_gradle_kts() {
        let dir = TempDir::new().unwrap();
        fs::write(
            dir.path().join("build.gradle.kts"),
            "plugins { id(\"org.springframework.boot\") }",
        )
        .unwrap();
        let stack = detect_stack(dir.path()).unwrap();
        assert_eq!(stack.language, Language::Java);
        assert_eq!(stack.framework, Framework::SpringBoot);
    }

    #[test]
    fn test_detect_no_language() {
        let dir = TempDir::new().unwrap();
        let stack = detect_stack(dir.path());
        assert!(stack.is_none());
    }
}
