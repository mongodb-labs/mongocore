use once_cell::sync::Lazy;
use tera::Tera;

use super::Language;

/// Global Tera template registry, initialized once on first use.
static TEMPLATES: Lazy<Tera> = Lazy::new(|| {
    let mut tera = Tera::default();

    // Register all templates as inline strings
    tera.add_raw_templates(vec![
        // Python templates
        ("python/find.tera", PYTHON_FIND),
        ("python/aggregate.tera", PYTHON_AGGREGATE),
        ("python/insert.tera", PYTHON_INSERT),

        // TypeScript templates
        ("typescript/find.tera", TYPESCRIPT_FIND),
        ("typescript/aggregate.tera", TYPESCRIPT_AGGREGATE),
        ("typescript/insert.tera", TYPESCRIPT_INSERT),

        // Go templates
        ("go/find.tera", GO_FIND),
        ("go/aggregate.tera", GO_AGGREGATE),
        ("go/insert.tera", GO_INSERT),

        // Java templates
        ("java/find.tera", JAVA_FIND),
        ("java/aggregate.tera", JAVA_AGGREGATE),
        ("java/insert.tera", JAVA_INSERT),
    ])
    .expect("Failed to register templates");

    tera
});

/// Renders a query template for the given language and operation.
///
/// # Arguments
/// * `language` - Target language (Python, TypeScript, Go, Java)
/// * `operation` - Operation name ("find", "aggregate", "insert")
/// * `context` - Template context with variables (host, database, collection, filter, etc.)
///
/// # Returns
/// Rendered code string, or error message if template not found or rendering fails.
pub fn render_query(
    language: Language,
    operation: &str,
    context: &tera::Context,
) -> Result<String, String> {
    let template_name = format!("{}/{}.tera", language.display_name(), operation);

    TEMPLATES
        .render(&template_name, context)
        .map_err(|e| format!("Template rendering failed: {}", e))
}

// ============================================================================
// Python Templates
// ============================================================================

const PYTHON_FIND: &str = r#"import asyncio
from mongocore import MongoCore

async def main():
    # Connect to MongoCore sidecar
    client = MongoCore("{{ host }}")
    await client.connect()

    try:
        # Execute find query
        results = await client.find(
            database="{{ database }}",
            collection="{{ collection }}",
            filter={{ filter }},
            {% if limit %}limit={{ limit }},{% endif %}
            {% if sort %}sort={{ sort }},{% endif %}
        )

        # Print results
        for doc in results:
            print(doc)
    finally:
        await client.close()

if __name__ == "__main__":
    asyncio.run(main())
"#;

const PYTHON_AGGREGATE: &str = r#"import asyncio
from mongocore import MongoCore

async def main():
    # Connect to MongoCore sidecar
    client = MongoCore("{{ host }}")
    await client.connect()

    try:
        # Execute aggregation pipeline
        results = await client.aggregate(
            database="{{ database }}",
            collection="{{ collection }}",
            pipeline={{ pipeline }},
        )

        # Print results
        for doc in results:
            print(doc)
    finally:
        await client.close()

if __name__ == "__main__":
    asyncio.run(main())
"#;

const PYTHON_INSERT: &str = r#"import asyncio
from mongocore import MongoCore

async def main():
    # Connect to MongoCore sidecar
    client = MongoCore("{{ host }}")
    await client.connect()

    try:
        # Insert document
        result = await client.insert_one(
            database="{{ database }}",
            collection="{{ collection }}",
            document={{ document }},
        )

        print(f"Inserted document with ID: {result['insertedId']}")
    finally:
        await client.close()

if __name__ == "__main__":
    asyncio.run(main())
"#;

// ============================================================================
// TypeScript Templates
// ============================================================================

const TYPESCRIPT_FIND: &str = r#"import { MongoCore } from 'mongocore';

async function main() {
  // Connect to MongoCore sidecar
  const client = new MongoCore('{{ host }}');
  await client.connect();

  try {
    // Execute find query
    const results = await client.find({
      database: '{{ database }}',
      collection: '{{ collection }}',
      filter: {{ filter }},
      {% if limit %}limit: {{ limit }},{% endif %}
      {% if sort %}sort: {{ sort }},{% endif %}
    });

    // Print results
    for (const doc of results) {
      console.log(doc);
    }
  } finally {
    await client.close();
  }
}

main().catch(console.error);
"#;

const TYPESCRIPT_AGGREGATE: &str = r#"import { MongoCore } from 'mongocore';

async function main() {
  // Connect to MongoCore sidecar
  const client = new MongoCore('{{ host }}');
  await client.connect();

  try {
    // Execute aggregation pipeline
    const results = await client.aggregate({
      database: '{{ database }}',
      collection: '{{ collection }}',
      pipeline: {{ pipeline }},
    });

    // Print results
    for (const doc of results) {
      console.log(doc);
    }
  } finally {
    await client.close();
  }
}

main().catch(console.error);
"#;

const TYPESCRIPT_INSERT: &str = r#"import { MongoCore } from 'mongocore';

async function main() {
  // Connect to MongoCore sidecar
  const client = new MongoCore('{{ host }}');
  await client.connect();

  try {
    // Insert document
    const result = await client.insertOne({
      database: '{{ database }}',
      collection: '{{ collection }}',
      document: {{ document }},
    });

    console.log(`Inserted document with ID: ${result.insertedId}`);
  } finally {
    await client.close();
  }
}

main().catch(console.error);
"#;

// ============================================================================
// Go Templates
// ============================================================================

const GO_FIND: &str = r#"package main

import (
	"context"
	"fmt"
	"log"

	"github.com/mongocore/mongocore-go"
)

func main() {
	// Connect to MongoCore sidecar
	client, err := mongocore.NewClient("{{ host }}")
	if err != nil {
		log.Fatal(err)
	}
	defer client.Close()

	ctx := context.Background()

	// Execute find query
	results, err := client.Find(ctx, &mongocore.FindRequest{
		Database:   "{{ database }}",
		Collection: "{{ collection }}",
		Filter:     `{{ filter }}`,
		{% if limit %}Limit:      {{ limit }},{% endif %}
		{% if sort %}Sort:       `{{ sort }}`,{% endif %}
	})
	if err != nil {
		log.Fatal(err)
	}

	// Print results
	for _, doc := range results {
		fmt.Println(doc)
	}
}
"#;

const GO_AGGREGATE: &str = r#"package main

import (
	"context"
	"fmt"
	"log"

	"github.com/mongocore/mongocore-go"
)

func main() {
	// Connect to MongoCore sidecar
	client, err := mongocore.NewClient("{{ host }}")
	if err != nil {
		log.Fatal(err)
	}
	defer client.Close()

	ctx := context.Background()

	// Execute aggregation pipeline
	results, err := client.Aggregate(ctx, &mongocore.AggregateRequest{
		Database:   "{{ database }}",
		Collection: "{{ collection }}",
		Pipeline:   `{{ pipeline }}`,
	})
	if err != nil {
		log.Fatal(err)
	}

	// Print results
	for _, doc := range results {
		fmt.Println(doc)
	}
}
"#;

const GO_INSERT: &str = r#"package main

import (
	"context"
	"fmt"
	"log"

	"github.com/mongocore/mongocore-go"
)

func main() {
	// Connect to MongoCore sidecar
	client, err := mongocore.NewClient("{{ host }}")
	if err != nil {
		log.Fatal(err)
	}
	defer client.Close()

	ctx := context.Background()

	// Insert document
	result, err := client.InsertOne(ctx, &mongocore.InsertOneRequest{
		Database:   "{{ database }}",
		Collection: "{{ collection }}",
		Document:   `{{ document }}`,
	})
	if err != nil {
		log.Fatal(err)
	}

	fmt.Printf("Inserted document with ID: %s\n", result.InsertedId)
}
"#;

// ============================================================================
// Java Templates
// ============================================================================

const JAVA_FIND: &str = r#"import com.mongocore.MongoCore;
import com.mongocore.FindRequest;
import org.bson.Document;

import java.util.List;

public class Main {
    public static void main(String[] args) {
        // Connect to MongoCore sidecar
        try (MongoCore client = new MongoCore("{{ host }}")) {
            // Execute find query
            FindRequest request = FindRequest.newBuilder()
                .setDatabase("{{ database }}")
                .setCollection("{{ collection }}")
                .setFilter("{{ filter }}")
                {% if limit %}.setLimit({{ limit }}){% endif %}
                {% if sort %}.setSort("{{ sort }}"){% endif %}
                .build();

            List<Document> results = client.find(request);

            // Print results
            for (Document doc : results) {
                System.out.println(doc.toJson());
            }
        } catch (Exception e) {
            e.printStackTrace();
        }
    }
}
"#;

const JAVA_AGGREGATE: &str = r#"import com.mongocore.MongoCore;
import com.mongocore.AggregateRequest;
import org.bson.Document;

import java.util.List;

public class Main {
    public static void main(String[] args) {
        // Connect to MongoCore sidecar
        try (MongoCore client = new MongoCore("{{ host }}")) {
            // Execute aggregation pipeline
            AggregateRequest request = AggregateRequest.newBuilder()
                .setDatabase("{{ database }}")
                .setCollection("{{ collection }}")
                .setPipeline("{{ pipeline }}")
                .build();

            List<Document> results = client.aggregate(request);

            // Print results
            for (Document doc : results) {
                System.out.println(doc.toJson());
            }
        } catch (Exception e) {
            e.printStackTrace();
        }
    }
}
"#;

const JAVA_INSERT: &str = r#"import com.mongocore.MongoCore;
import com.mongocore.InsertOneRequest;
import com.mongocore.InsertOneResult;

public class Main {
    public static void main(String[] args) {
        // Connect to MongoCore sidecar
        try (MongoCore client = new MongoCore("{{ host }}")) {
            // Insert document
            InsertOneRequest request = InsertOneRequest.newBuilder()
                .setDatabase("{{ database }}")
                .setCollection("{{ collection }}")
                .setDocument("{{ document }}")
                .build();

            InsertOneResult result = client.insertOne(request);

            System.out.println("Inserted document with ID: " + result.getInsertedId());
        } catch (Exception e) {
            e.printStackTrace();
        }
    }
}
"#;

#[cfg(test)]
mod tests {
    use super::*;
    use tera::Context;

    #[test]
    fn test_python_find_renders() {
        let mut ctx = Context::new();
        ctx.insert("host", "localhost:50051");
        ctx.insert("database", "testdb");
        ctx.insert("collection", "users");
        ctx.insert("filter", r#"{"age": {"$gt": 18}}"#);
        ctx.insert("limit", &10);

        let result = render_query(Language::Python, "find", &ctx);
        assert!(result.is_ok());
        let code = result.unwrap();
        assert!(code.contains("MongoCore"));
        assert!(code.contains("localhost:50051"));
        assert!(code.contains("testdb"));
        assert!(code.contains("users"));
        assert!(code.contains(r#"{"age": {"$gt": 18}}"#));
        assert!(code.contains("limit=10"));
    }

    #[test]
    fn test_typescript_aggregate_renders() {
        let mut ctx = Context::new();
        ctx.insert("host", "localhost:50051");
        ctx.insert("database", "analytics");
        ctx.insert("collection", "events");
        ctx.insert("pipeline", r#"[{"$match": {"type": "click"}}, {"$group": {"_id": "$userId", "count": {"$sum": 1}}}]"#);

        let result = render_query(Language::TypeScript, "aggregate", &ctx);
        assert!(result.is_ok());
        let code = result.unwrap();
        assert!(code.contains("MongoCore"));
        assert!(code.contains("localhost:50051"));
        assert!(code.contains("analytics"));
        assert!(code.contains("events"));
        assert!(code.contains(r#"[{"$match": {"type": "click"}}"#));
    }

    #[test]
    fn test_go_find_renders() {
        let mut ctx = Context::new();
        ctx.insert("host", "localhost:50051");
        ctx.insert("database", "mydb");
        ctx.insert("collection", "items");
        ctx.insert("filter", r#"{"status": "active"}"#);

        let result = render_query(Language::Go, "find", &ctx);
        assert!(result.is_ok());
        let code = result.unwrap();
        assert!(code.contains("mongocore.NewClient"));
        assert!(code.contains("localhost:50051"));
        assert!(code.contains("mydb"));
        assert!(code.contains("items"));
        assert!(code.contains(r#"{"status": "active"}"#));
    }

    #[test]
    fn test_java_insert_renders() {
        let mut ctx = Context::new();
        ctx.insert("host", "localhost:50051");
        ctx.insert("database", "appdb");
        ctx.insert("collection", "posts");
        ctx.insert("document", r#"{"title": "Hello", "content": "World"}"#);

        let result = render_query(Language::Java, "insert", &ctx);
        assert!(result.is_ok());
        let code = result.unwrap();
        assert!(code.contains("MongoCore"));
        assert!(code.contains("localhost:50051"));
        assert!(code.contains("appdb"));
        assert!(code.contains("posts"));
        assert!(code.contains(r#"{"title": "Hello", "content": "World"}"#));
        assert!(code.contains("insertOne"));
    }

    #[test]
    fn test_invalid_template_returns_error() {
        let ctx = Context::new();
        let result = render_query(Language::Python, "nonexistent", &ctx);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Template rendering failed"));
    }

    #[test]
    fn test_all_languages_find_template_exists() {
        let mut ctx = Context::new();
        // Populate minimal required context to avoid rendering errors
        ctx.insert("host", "localhost:50051");
        ctx.insert("database", "test");
        ctx.insert("collection", "test");
        ctx.insert("filter", "{}");

        for lang in [Language::Python, Language::TypeScript, Language::Go, Language::Java] {
            let result = render_query(lang, "find", &ctx);
            assert!(
                result.is_ok(),
                "Failed to render {} find template: {:?}",
                lang.display_name(),
                result.unwrap_err()
            );
        }
    }

    #[test]
    fn test_all_operations_template_exists() {
        let mut ctx = Context::new();
        // Populate minimal required context
        ctx.insert("host", "localhost:50051");
        ctx.insert("database", "test");
        ctx.insert("collection", "test");
        ctx.insert("filter", "{}");
        ctx.insert("pipeline", "[]");
        ctx.insert("document", "{}");

        for op in ["find", "aggregate", "insert"] {
            let result = render_query(Language::Python, op, &ctx);
            assert!(
                result.is_ok(),
                "Failed to render Python {} template: {:?}",
                op,
                result.unwrap_err()
            );
        }
    }
}
