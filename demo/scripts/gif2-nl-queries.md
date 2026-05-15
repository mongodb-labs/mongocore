# GIF 2: Natural Language Queries

## Pre-conditions
- MongoDB running with `movies` collection loaded (from GIF 1)
- Compiled query cache cleared: drop `__mongocore` database and restart MongoCore
- LLM configured (Claude or OpenAI API key set)

## Recording

Start: `asciinema rec --cols 100 --rows 30 --idle-time-limit 2 recordings/gif2-nl-queries.cast`

## Prompts

### Prompt 1: Cold query (LLM call)
```
What are the top-rated sci-fi movies from the 1990s in the demo database?
```

Wait for results. Note the response time (~1-2s for LLM translation + execution).

### Prompt 2: Warm query (cache hit)
```
What about horror movies from the 2000s?
```

Wait for results. Note the sub-second response (template cache hit, no LLM).

## End

Stop recording (Ctrl+D or `exit`).

Convert to GIF: `agg --speed 1.2 recordings/gif2-nl-queries.cast gifs/gif2-nl-queries.gif --theme monokai`

Or use asciinema player for web embedding.

## Expected Duration: ~30-35 seconds of content
