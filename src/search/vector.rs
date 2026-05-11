use bson::{doc, Document};

pub struct VectorSearchBuilder;

impl VectorSearchBuilder {
    /// Build a $vectorSearch aggregation pipeline stage.
    pub fn build_pipeline(
        index_name: &str,
        field_path: &str,
        query_vector: Vec<f64>,
        num_candidates: i64,
        limit: i64,
    ) -> Vec<Document> {
        vec![
            doc! {
                "$vectorSearch": {
                    "index": index_name,
                    "path": field_path,
                    "queryVector": query_vector,
                    "numCandidates": num_candidates,
                    "limit": limit,
                }
            },
            doc! {
                "$addFields": {
                    "search_score": { "$meta": "vectorSearchScore" }
                }
            },
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_pipeline_structure() {
        let vector = vec![0.1, 0.2, 0.3];
        let pipeline = VectorSearchBuilder::build_pipeline(
            "my_index",
            "embedding",
            vector.clone(),
            100,
            10,
        );

        assert_eq!(pipeline.len(), 2);

        // Verify $vectorSearch stage
        let vs_stage = &pipeline[0];
        let vs = vs_stage.get_document("$vectorSearch").unwrap();
        assert_eq!(vs.get_str("index").unwrap(), "my_index");
        assert_eq!(vs.get_str("path").unwrap(), "embedding");
        assert_eq!(vs.get_i64("numCandidates").unwrap(), 100);
        assert_eq!(vs.get_i64("limit").unwrap(), 10);

        let query_vec = vs.get_array("queryVector").unwrap();
        assert_eq!(query_vec.len(), 3);
    }

    #[test]
    fn test_build_pipeline_add_fields_stage() {
        let pipeline = VectorSearchBuilder::build_pipeline(
            "idx",
            "vec_field",
            vec![1.0, 2.0],
            50,
            5,
        );

        let add_fields = &pipeline[1];
        let fields = add_fields.get_document("$addFields").unwrap();
        let score = fields.get_document("search_score").unwrap();
        assert_eq!(score.get_str("$meta").unwrap(), "vectorSearchScore");
    }

    #[test]
    fn test_build_pipeline_empty_vector() {
        let pipeline = VectorSearchBuilder::build_pipeline(
            "test_index",
            "path",
            vec![],
            10,
            5,
        );

        let vs = pipeline[0].get_document("$vectorSearch").unwrap();
        let query_vec = vs.get_array("queryVector").unwrap();
        assert!(query_vec.is_empty());
    }
}
