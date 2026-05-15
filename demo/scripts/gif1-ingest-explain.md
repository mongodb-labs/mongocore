# GIF 1: Ingest + Explain Last

## Pre-conditions
- MongoDB running (`just docker-up`)
- MongoCore built and configured as MCP server in Claude Code
- No existing `movies` collection in the default database

## Recording

Start: `asciinema rec --cols 100 --rows 30 --idle-time-limit 2 recordings/gif1-ingest-explain.cast`

## Prompts

### Prompt 1: Ingest
```
Ingest this CSV into MongoDB as a new collection called movies and rename vote_average to rating: https://raw.githubusercontent.com/mongodb-labs/mongocore/datasets/demo/data/movies_dataset.csv
 .
```

Wait for completion (expect ~4800 documents ingested message).

### Prompt 2: Explain
```
Show me the Python code to do that ingestion
```

Wait for `explain_last` output showing Python function.

## End

Stop recording (Ctrl+D or `exit`).

Convert to GIF: `agg --speed 1.2 recordings/gif1-ingest-explain.cast gifs/gif1-ingest-explain.gif --theme monokai`

Or use asciinema player for web embedding.

## Expected Duration: ~30-35 seconds of content
