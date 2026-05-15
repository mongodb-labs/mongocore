# GIF 4: ETL + Explain Session

## Pre-conditions
- MongoDB running
- No existing `box_office` collection in the default database
- Fresh MCP session (restart Claude Code or clear session)

## Recording

Start: `asciinema rec --cols 100 --rows 30 --idle-time-limit 2 recordings/gif4-etl-explain.cast`

## Prompts

### Prompt 1: Ingest with transform
```
Ingest this CSV into the box_office collection: https://raw.githubusercontent.com/mongodb-labs/mongocore/datasets/demo/data/box_office.csv
Calculate a profit field as DomesticGross + ForeignGross - Budget.
```

Wait for completion (~970 documents ingested).

### Prompt 2: Create index
```
Create an index on genre and profit (descending) for the box_office collection.
```

Wait for index creation confirmation.

### Prompt 3: Verify query
```
What are the most profitable Action movies?
```

Wait for results showing movies with computed profit field.

### Prompt 4: Explain session
```
Show me the full Python script to reproduce everything I just did.
```

Wait for `explain_session` output showing complete Python script.

## End

Stop recording (Ctrl+D or `exit`).

Convert to GIF: `agg --speed 1.2 recordings/gif4-etl-explain.cast gifs/gif4-etl-explain.gif --theme monokai`

Or use asciinema player for web embedding.

## Expected Duration: ~35-40 seconds of content
