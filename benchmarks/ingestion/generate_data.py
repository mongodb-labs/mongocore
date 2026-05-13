"""Generate benchmark datasets at target file sizes (1MB, 10MB, 100MB)."""

import csv
import json
import random
import string
from pathlib import Path

DATA_DIR = Path(__file__).parent / "data"
DATA_DIR.mkdir(exist_ok=True)

# Target sizes in bytes
TARGETS = [
    ("1mb", 1_000_000),
    ("10mb", 10_000_000),
    ("100mb", 100_000_000),
]

# Each row is approximately 120 bytes in CSV, 180 bytes in NDJSON
APPROX_ROW_SIZE_CSV = 120
APPROX_ROW_SIZE_NDJSON = 180


def random_string(length=10):
    return "".join(random.choices(string.ascii_lowercase, k=length))


def generate_row(i):
    return {
        "id": i,
        "name": f"User {random_string(8)}",
        "email": f"{random_string(6)}@example.com",
        "age": random.randint(18, 80),
        "score": round(random.uniform(0, 100), 2),
        "created_at": f"2024-{random.randint(1,12):02d}-{random.randint(1,28):02d}T{random.randint(0,23):02d}:{random.randint(0,59):02d}:00Z",
        "tags": [random_string(5) for _ in range(3)],
    }


def main():
    for label, target_bytes in TARGETS:
        # CSV
        csv_path = DATA_DIR / f"bench_{label}.csv"
        if not csv_path.exists():
            num_rows = target_bytes // APPROX_ROW_SIZE_CSV
            print(f"Generating {csv_path} (~{target_bytes // 1_000_000}MB, {num_rows:,} rows)...")
            rows = [generate_row(i) for i in range(num_rows)]
            with open(csv_path, "w", newline="") as f:
                writer = csv.DictWriter(f, fieldnames=rows[0].keys())
                writer.writeheader()
                writer.writerows(rows)
            actual_size = csv_path.stat().st_size
            print(f"  → {actual_size / 1_000_000:.1f} MB ({num_rows:,} rows)")

        # NDJSON
        ndjson_path = DATA_DIR / f"bench_{label}.ndjson"
        if not ndjson_path.exists():
            num_rows = target_bytes // APPROX_ROW_SIZE_NDJSON
            print(f"Generating {ndjson_path} (~{target_bytes // 1_000_000}MB, {num_rows:,} rows)...")
            with open(ndjson_path, "w") as f:
                for i in range(num_rows):
                    f.write(json.dumps(generate_row(i)) + "\n")
            actual_size = ndjson_path.stat().st_size
            print(f"  → {actual_size / 1_000_000:.1f} MB ({num_rows:,} rows)")

    print("\nDone generating benchmark data.")
    print(f"Files in {DATA_DIR}:")
    for f in sorted(DATA_DIR.glob("*")):
        print(f"  {f.name}: {f.stat().st_size / 1_000_000:.1f} MB")


if __name__ == "__main__":
    main()
