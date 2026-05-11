use bson::{doc, Document};

pub struct FulltextSearchBuilder;

impl FulltextSearchBuilder {
    /// Build a $search aggregation pipeline for Atlas full-text search.
    pub fn build_pipeline(
        index_name: &str,
        query: &str,
        search_fields: &[&str],
        limit: i64,
    ) -> Vec<Document> {
        let fields: Vec<bson::Bson> = search_fields
            .iter()
            .map(|f| bson::Bson::String(f.to_string()))
            .collect();

        vec![
            doc! {
                "$search": {
                    "index": index_name,
                    "text": {
                        "query": query,
                        "path": fields,
                    }
                }
            },
            doc! {
                "$addFields": {
                    "search_score": { "$meta": "searchScore" }
                }
            },
            doc! { "$limit": limit },
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_pipeline_structure() {
        let pipeline =
            FulltextSearchBuilder::build_pipeline("default", "hello world", &["title", "body"], 10);

        assert_eq!(pipeline.len(), 3);

        // Verify $search stage
        let search_stage = &pipeline[0];
        let search = search_stage.get_document("$search").unwrap();
        assert_eq!(search.get_str("index").unwrap(), "default");

        let text = search.get_document("text").unwrap();
        assert_eq!(text.get_str("query").unwrap(), "hello world");

        let path = text.get_array("path").unwrap();
        assert_eq!(path.len(), 2);
    }

    #[test]
    fn test_build_pipeline_add_fields_stage() {
        let pipeline = FulltextSearchBuilder::build_pipeline("idx", "test", &["field1"], 5);

        let add_fields = &pipeline[1];
        let fields = add_fields.get_document("$addFields").unwrap();
        let score = fields.get_document("search_score").unwrap();
        assert_eq!(score.get_str("$meta").unwrap(), "searchScore");
    }

    #[test]
    fn test_build_pipeline_limit_stage() {
        let pipeline = FulltextSearchBuilder::build_pipeline("idx", "query", &["*"], 25);

        let limit_stage = &pipeline[2];
        assert_eq!(limit_stage.get_i64("$limit").unwrap(), 25);
    }

    #[test]
    fn test_build_pipeline_single_field() {
        let pipeline =
            FulltextSearchBuilder::build_pipeline("my_index", "search term", &["content"], 10);

        let search = pipeline[0].get_document("$search").unwrap();
        let text = search.get_document("text").unwrap();
        let path = text.get_array("path").unwrap();
        assert_eq!(path.len(), 1);
        assert_eq!(path[0].as_str().unwrap(), "content");
    }
}
