# GIF 3: Request + Transactional Pipelines

## Pre-conditions
- MongoDB running with `movies` collection loaded (from GIF 1)

## Recording

Start: `asciinema rec --cols 100 --rows 30 --idle-time-limit 2 recordings/gif3-pipelines.cast`

## Prompts

### Prompt 1: Request pipeline (batch update)
```
For every movie in the movies collection, set a field 'source' to 'movies_import' and 'imported_at' to today's date. Do it in a single batch.
```

Wait for completion (expect count of updated documents).

### Prompt 2: Transactional pipeline (atomic multi-step)
```
In a transaction: find the highest-rated movie, then update it to set 'featured: true' and 'featured_at' to today's date. Roll back if anything fails.
```

Wait for completion (expect transaction result with find + update steps).

## End

Stop recording (Ctrl+D or `exit`).

Convert to GIF: `agg --speed 1.2 recordings/gif3-pipelines.cast gifs/gif3-pipelines.gif --theme monokai`

Or use asciinema player for web embedding.

## Expected Duration: ~35-40 seconds of content
