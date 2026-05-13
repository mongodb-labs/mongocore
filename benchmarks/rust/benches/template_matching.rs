use criterion::{criterion_group, criterion_main, Criterion};
use mongocore::compiled::template_registry::TemplateRegistry;
use mongocore::compiled::{LlmTemplate, LlmTemplateParameter, ParameterType};

fn bench_template_match_hit(c: &mut Criterion) {
    let registry = TemplateRegistry::new();
    let template = LlmTemplate {
        intent_pattern: "find {{cuisine}} restaurants in {{location}}".to_string(),
        parameters: vec![
            LlmTemplateParameter {
                name: "cuisine".to_string(),
                value: serde_json::json!("Italian"),
                param_type: ParameterType::String,
            },
            LlmTemplateParameter {
                name: "location".to_string(),
                value: serde_json::json!("Manhattan"),
                param_type: ParameterType::String,
            },
        ],
        mql_pattern: serde_json::json!({"cuisine": "{{cuisine}}", "borough": "{{location}}"}),
    };
    registry.register(&template, "filter", "sample_restaurants", "restaurants");

    c.bench_function("template_registry_match_hit", |b| {
        b.iter(|| {
            registry.try_match(
                "find Chinese restaurants in Brooklyn",
                "sample_restaurants",
                "restaurants",
            )
        })
    });
}

fn bench_template_match_miss(c: &mut Criterion) {
    let registry = TemplateRegistry::new();
    let template = LlmTemplate {
        intent_pattern: "find {{cuisine}} restaurants in {{location}}".to_string(),
        parameters: vec![
            LlmTemplateParameter {
                name: "cuisine".to_string(),
                value: serde_json::json!("Italian"),
                param_type: ParameterType::String,
            },
            LlmTemplateParameter {
                name: "location".to_string(),
                value: serde_json::json!("Manhattan"),
                param_type: ParameterType::String,
            },
        ],
        mql_pattern: serde_json::json!({"cuisine": "{{cuisine}}", "borough": "{{location}}"}),
    };
    registry.register(&template, "filter", "sample_restaurants", "restaurants");

    c.bench_function("template_registry_match_miss", |b| {
        b.iter(|| {
            registry.try_match(
                "count restaurants by borough",
                "sample_restaurants",
                "restaurants",
            )
        })
    });
}

fn bench_template_match_multiple_templates(c: &mut Criterion) {
    let registry = TemplateRegistry::new();

    // Register multiple templates to simulate realistic usage
    for i in 0..10 {
        let template = LlmTemplate {
            intent_pattern: format!("find {{{{param{}}}}} documents", i),
            parameters: vec![LlmTemplateParameter {
                name: format!("param{}", i),
                value: serde_json::json!("value"),
                param_type: ParameterType::String,
            }],
            mql_pattern: serde_json::json!({"field": format!("{{{{param{}}}}}", i)}),
        };
        registry.register(&template, "filter", "test_db", "test_coll");
    }

    c.bench_function("template_registry_match_with_multiple", |b| {
        b.iter(|| {
            registry.try_match(
                "find test documents",
                "test_db",
                "test_coll",
            )
        })
    });
}

criterion_group!(benches, bench_template_match_hit, bench_template_match_miss, bench_template_match_multiple_templates);
criterion_main!(benches);
