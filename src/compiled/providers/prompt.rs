use super::TranslationContext;

/// Build the system/user prompt for NL→MQL translation with routing and templates.
pub fn build_translation_prompt(
    intent: &str,
    database: &str,
    collection: &str,
    context: &TranslationContext,
) -> String {
    let mut prompt = format!(
        "Translate this natural language query into a MongoDB query.\n\n\
         Database: {}\nCollection: {}\nIntent: \"{}\"\n\n",
        database, collection, intent
    );
    if let Some(ref schema) = context.schema_hint {
        prompt.push_str(&format!("Schema: {}\n\n", schema));
    }
    if !context.sample_documents.is_empty() {
        prompt.push_str("Sample documents:\n");
        for doc in &context.sample_documents {
            prompt.push_str(&format!("  {}\n", doc));
        }
        prompt.push('\n');
    }
    if !context.available_indexes.is_empty() {
        prompt.push_str("Available indexes:\n");
        for idx in &context.available_indexes {
            prompt.push_str(&format!("  {}\n", idx));
        }
        prompt.push('\n');
    }
    prompt.push_str(
        "Respond with ONLY valid JSON containing:\n\
         1. \"type\": \"find\" or \"aggregate\"\n\
         2. \"method\": The best execution method:\n\
            - \"filter\" — structured queries with field-based conditions\n\
            - \"aggregate\" — group-by, counts, averages, joins, top-N\n\
            - \"vector_search\" — semantic/meaning-based queries\n\
            - \"fulltext\" — keyword text search\n\
            - \"geo\" — proximity/location queries\n\
         3. The query (\"filter\" for find/geo, \"pipeline\" for aggregate, \"search_query\" for search methods)\n\
         4. \"template\" (optional): parameterized version for cache reuse:\n\
            - \"intent_pattern\": the query with variable parts as {{param_name}}\n\
            - \"parameters\": [{\"name\": \"...\", \"value\": ..., \"param_type\": \"String\"|\"Number\"|\"Boolean\"}]\n\
            - \"mql_pattern\": the MQL with {{param_name}} placeholders\n\n\
         No explanation, no markdown. Only valid JSON.",
    );
    prompt
}
