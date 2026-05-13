// Benchmark Go MongoDB native driver for comparison against MongoCore.
package main

import (
	"context"
	"encoding/json"
	"fmt"
	"math"
	"os"
	"path/filepath"
	"runtime"
	"sort"
	"time"

	"go.mongodb.org/mongo-driver/v2/bson"
	"go.mongodb.org/mongo-driver/v2/mongo"
	"go.mongodb.org/mongo-driver/v2/mongo/options"
)

type Config struct {
	MongoDBURI     string `json:"mongodb_uri"`
	MongoCoreAddr  string `json:"mongocore_address"`
	Database       string `json:"database"`
	MinTimeSecs    int    `json:"min_time_secs"`
	MaxIterations  int    `json:"max_iterations"`
	MaxTimeSecs    int    `json:"max_time_secs"`
	WarmupIters    map[string]int `json:"warmup_iterations"`
}

type SystemInfo struct {
	OS              string `json:"os"`
	Arch            string `json:"arch"`
	CPUs            int    `json:"cpus"`
	MongoCoreVer    string `json:"mongocore_version"`
	Driver          string `json:"driver"`
}

type Percentiles struct {
	P10 float64 `json:"p10"`
	P25 float64 `json:"p25"`
	P50 float64 `json:"p50"`
	P75 float64 `json:"p75"`
	P90 float64 `json:"p90"`
	P95 float64 `json:"p95"`
	P99 float64 `json:"p99"`
}

type BenchResult struct {
	Benchmark       string      `json:"benchmark"`
	Category        string      `json:"category"`
	Driver          string      `json:"driver"`
	DatasetBytes    int         `json:"dataset_size_bytes"`
	BatchSize       int         `json:"batch_size"`
	Iterations      int         `json:"iterations"`
	TotalTimeSecs   float64     `json:"total_time_secs"`
	OpsPerSec       float64     `json:"ops_per_sec"`
	MBPerSec        float64     `json:"mb_per_sec"`
	Percentiles     Percentiles `json:"percentiles"`
	Timestamp       string      `json:"timestamp"`
	System          SystemInfo  `json:"system"`
}

func getSystemInfo() SystemInfo {
	return SystemInfo{
		OS:           runtime.GOOS,
		Arch:         runtime.GOARCH,
		CPUs:         runtime.NumCPU(),
		MongoCoreVer: "native",
		Driver:       "mongo-go-driver",
	}
}

func percentile(data []float64, pct int) float64 {
	idx := int(math.Ceil(float64(len(data))*float64(pct)/100.0)) - 1
	if idx < 0 {
		idx = 0
	}
	if idx >= len(data) {
		idx = len(data) - 1
	}
	return data[idx]
}

func runBenchmark(
	name, category string,
	setupFn func(*mongo.Database) error,
	beforeTaskFn func(*mongo.Database) error,
	taskFn func(*mongo.Database) error,
	afterTaskFn func(*mongo.Database) error,
	teardownFn func(*mongo.Database) error,
	datasetBytes, batchSize int,
	config Config,
) BenchResult {
	ctx := context.Background()
	client, err := mongo.Connect(options.Client().ApplyURI(config.MongoDBURI))
	if err != nil {
		panic(err)
	}
	defer client.Disconnect(ctx)

	db := client.Database(config.Database)

	if err := setupFn(db); err != nil {
		panic(err)
	}

	// Warmup
	warmup := config.WarmupIters["go"]
	for i := 0; i < warmup; i++ {
		if err := beforeTaskFn(db); err != nil {
			panic(err)
		}
		if err := taskFn(db); err != nil {
			panic(err)
		}
		if err := afterTaskFn(db); err != nil {
			panic(err)
		}
	}

	// Timed iterations
	times := []float64{}
	totalTime := 0.0
	iteration := 0

	for totalTime < float64(config.MinTimeSecs) || iteration < 5 {
		if iteration >= config.MaxIterations || totalTime >= float64(config.MaxTimeSecs) {
			break
		}

		if err := beforeTaskFn(db); err != nil {
			panic(err)
		}

		start := time.Now()
		if err := taskFn(db); err != nil {
			panic(err)
		}
		elapsed := time.Since(start).Seconds()

		if err := afterTaskFn(db); err != nil {
			panic(err)
		}

		times = append(times, elapsed)
		totalTime += elapsed
		iteration++
	}

	if err := teardownFn(db); err != nil {
		panic(err)
	}

	// Calculate metrics
	sort.Float64s(times)
	median := times[len(times)/2]
	opsPerSec := float64(batchSize) / median
	mbPerSec := float64(datasetBytes) / median / 1_000_000

	result := BenchResult{
		Benchmark:     name,
		Category:      category,
		Driver:        "mongo-go-driver",
		DatasetBytes:  datasetBytes,
		BatchSize:     batchSize,
		Iterations:    len(times),
		TotalTimeSecs: math.Round(totalTime*1000) / 1000,
		OpsPerSec:     math.Round(opsPerSec*10) / 10,
		MBPerSec:      math.Round(mbPerSec*1000) / 1000,
		Percentiles: Percentiles{
			P10: math.Round(percentile(times, 10)*1_000_000) / 1_000_000,
			P25: math.Round(percentile(times, 25)*1_000_000) / 1_000_000,
			P50: math.Round(median*1_000_000) / 1_000_000,
			P75: math.Round(percentile(times, 75)*1_000_000) / 1_000_000,
			P90: math.Round(percentile(times, 90)*1_000_000) / 1_000_000,
			P95: math.Round(percentile(times, 95)*1_000_000) / 1_000_000,
			P99: math.Round(percentile(times, 99)*1_000_000) / 1_000_000,
		},
		Timestamp: time.Now().UTC().Format(time.RFC3339),
		System:    getSystemInfo(),
	}

	fmt.Printf("  %s: %.0f ops/s, %.2f MB/s (%d iterations)\n", name, opsPerSec, mbPerSec, len(times))
	return result
}

func main() {
	fmt.Println("=== Go MongoDB native driver benchmarks ===")

	// Load config
	configPath := filepath.Join("..", "common.json")
	configBytes, err := os.ReadFile(configPath)
	if err != nil {
		panic(err)
	}
	var config Config
	if err := json.Unmarshal(configBytes, &config); err != nil {
		panic(err)
	}

	// Load test documents
	dataDir := filepath.Join("..", "..", "data")
	smallDocBytes, err := os.ReadFile(filepath.Join(dataDir, "small_doc.json"))
	if err != nil {
		panic(fmt.Sprintf("Failed to read small_doc.json: %v", err))
	}
	tweetDocBytes, err := os.ReadFile(filepath.Join(dataDir, "tweet.json"))
	if err != nil {
		panic(fmt.Sprintf("Failed to read tweet.json: %v", err))
	}
	largeDocBytes, err := os.ReadFile(filepath.Join(dataDir, "large_doc.json"))
	if err != nil {
		panic(fmt.Sprintf("Failed to read large_doc.json: %v", err))
	}

	var smallDoc, tweetDoc, largeDoc bson.M
	json.Unmarshal(smallDocBytes, &smallDoc)
	json.Unmarshal(tweetDocBytes, &tweetDoc)
	json.Unmarshal(largeDocBytes, &largeDoc)

	smallSize := len(smallDocBytes)
	tweetSize := len(tweetDocBytes)
	largeSize := len(largeDocBytes)

	results := []BenchResult{}

	// Run Command (batch 10,000 hello commands per iteration)
	results = append(results, runBenchmark(
		"run_command", "single_doc",
		func(db *mongo.Database) error { return nil },
		func(db *mongo.Database) error { return nil },
		func(db *mongo.Database) error {
			for i := 0; i < 10_000; i++ {
				if err := db.RunCommand(context.Background(), bson.D{{Key: "hello", Value: 1}}).Err(); err != nil {
					return err
				}
			}
			return nil
		},
		func(db *mongo.Database) error { return nil },
		func(db *mongo.Database) error { return nil },
		10_000*100, 10_000, config,
	))

	// Find One by ID (batch 10,000 finds per iteration)
	results = append(results, runBenchmark(
		"find_one_by_id", "single_doc",
		func(db *mongo.Database) error {
			coll := db.Collection("bench_find")
			coll.Drop(context.Background())
			doc := bson.M{"_id": bson.ObjectID{0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1}}
			for k, v := range tweetDoc {
				if k != "_id" {
					doc[k] = v
				}
			}
			_, err := coll.InsertOne(context.Background(), doc)
			return err
		},
		func(db *mongo.Database) error { return nil },
		func(db *mongo.Database) error {
			coll := db.Collection("bench_find")
			for i := 0; i < 10_000; i++ {
				if err := coll.FindOne(context.Background(), bson.M{"_id": bson.ObjectID{0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1}}).Err(); err != nil {
					return err
				}
			}
			return nil
		},
		func(db *mongo.Database) error { return nil },
		func(db *mongo.Database) error {
			return db.Collection("bench_find").Drop(context.Background())
		},
		10_000*tweetSize, 10_000, config,
	))

	// InsertOne Small (batch 10,000 inserts per iteration)
	results = append(results, runBenchmark(
		"insert_one_small", "single_doc",
		func(db *mongo.Database) error { return nil },
		func(db *mongo.Database) error {
			return db.Collection("bench_insert_small").Drop(context.Background())
		},
		func(db *mongo.Database) error {
			for i := 0; i < 10_000; i++ {
				doc := bson.M{"_id": bson.NewObjectID()}
				for k, v := range smallDoc {
					if k != "_id" {
						doc[k] = v
					}
				}
				if _, err := db.Collection("bench_insert_small").InsertOne(context.Background(), doc); err != nil {
					return err
				}
			}
			return nil
		},
		func(db *mongo.Database) error { return nil },
		func(db *mongo.Database) error { return nil },
		10_000*smallSize, 10_000, config,
	))

	// InsertOne Large (batch 10 inserts per iteration)
	results = append(results, runBenchmark(
		"insert_one_large", "single_doc",
		func(db *mongo.Database) error { return nil },
		func(db *mongo.Database) error {
			return db.Collection("bench_insert_large").Drop(context.Background())
		},
		func(db *mongo.Database) error {
			for i := 0; i < 10; i++ {
				doc := bson.M{"_id": bson.NewObjectID()}
				for k, v := range largeDoc {
					if k != "_id" {
						doc[k] = v
					}
				}
				if _, err := db.Collection("bench_insert_large").InsertOne(context.Background(), doc); err != nil {
					return err
				}
			}
			return nil
		},
		func(db *mongo.Database) error { return nil },
		func(db *mongo.Database) error { return nil },
		10*largeSize, 10, config,
	))

	// Bulk Insert Small (10,000 docs per iteration)
	results = append(results, runBenchmark(
		"bulk_insert_small", "multi_doc",
		func(db *mongo.Database) error { return nil },
		func(db *mongo.Database) error {
			return db.Collection("bench_bulk").Drop(context.Background())
		},
		func(db *mongo.Database) error {
			docs := make([]interface{}, 10_000)
			for i := 0; i < 10_000; i++ {
				doc := bson.M{"_id": bson.NewObjectID()}
				for k, v := range smallDoc {
					if k != "_id" {
						doc[k] = v
					}
				}
				docs[i] = doc
			}
			_, err := db.Collection("bench_bulk").InsertMany(context.Background(), docs)
			return err
		},
		func(db *mongo.Database) error { return nil },
		func(db *mongo.Database) error { return nil },
		smallSize*10_000, 10_000, config,
	))

	// Find Many (10,000 docs)
	results = append(results, runBenchmark(
		"find_many", "multi_doc",
		func(db *mongo.Database) error {
			coll := db.Collection("bench_find_many")
			coll.Drop(context.Background())
			docs := make([]interface{}, 10_000)
			for i := 0; i < 10_000; i++ {
				doc := bson.M{"_id": bson.NewObjectID()}
				for k, v := range smallDoc {
					if k != "_id" {
						doc[k] = v
					}
				}
				docs[i] = doc
			}
			_, err := coll.InsertMany(context.Background(), docs)
			return err
		},
		func(db *mongo.Database) error { return nil },
		func(db *mongo.Database) error {
			coll := db.Collection("bench_find_many")
			cursor, err := coll.Find(context.Background(), bson.M{})
			if err != nil {
				return err
			}
			defer cursor.Close(context.Background())
			var results []bson.M
			return cursor.All(context.Background(), &results)
		},
		func(db *mongo.Database) error { return nil },
		func(db *mongo.Database) error {
			return db.Collection("bench_find_many").Drop(context.Background())
		},
		smallSize*10_000, 10_000, config,
	))

	// Save results
	resultsDir := filepath.Join("..", "..", "results")
	os.MkdirAll(resultsDir, 0755)
	outputPath := filepath.Join(resultsDir, "go_native.json")
	resultsJSON, _ := json.MarshalIndent(results, "", "  ")
	if err := os.WriteFile(outputPath, resultsJSON, 0644); err != nil {
		panic(err)
	}
	fmt.Printf("\nResults saved to %s\n", outputPath)
}
