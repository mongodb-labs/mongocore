// Benchmark MongoCore Go client (via gRPC sidecar).
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

	"github.com/rozza/mongocore/clients/go/mongocore"
	"go.mongodb.org/mongo-driver/v2/bson"
	
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
		MongoCoreVer: "0.6.0",
		Driver:       "mongocore+go",
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
	setupFn func(*mongocore.Client) error,
	beforeTaskFn func(*mongocore.Client) error,
	taskFn func(*mongocore.Client) error,
	afterTaskFn func(*mongocore.Client) error,
	teardownFn func(*mongocore.Client) error,
	datasetBytes, batchSize int,
	config Config,
) BenchResult {
	client := mongocore.MongoClientTCP(config.MongoCoreAddr)
	ctx := context.Background()
	if err := client.Connect(ctx); err != nil {
		panic(err)
	}
	defer client.Close()

	if err := setupFn(client); err != nil {
		panic(err)
	}

	// Warmup
	warmup := config.WarmupIters["go"]
	for i := 0; i < warmup; i++ {
		if err := beforeTaskFn(client); err != nil {
			panic(err)
		}
		if err := taskFn(client); err != nil {
			panic(err)
		}
		if err := afterTaskFn(client); err != nil {
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

		if err := beforeTaskFn(client); err != nil {
			panic(err)
		}

		start := time.Now()
		if err := taskFn(client); err != nil {
			panic(err)
		}
		elapsed := time.Since(start).Seconds()

		if err := afterTaskFn(client); err != nil {
			panic(err)
		}

		times = append(times, elapsed)
		totalTime += elapsed
		iteration++
	}

	if err := teardownFn(client); err != nil {
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
		Driver:        "mongocore+go",
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
	fmt.Println("=== MongoCore+Go benchmarks ===")

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

	var smallDoc, tweetDoc, largeDoc map[string]interface{}
	json.Unmarshal(smallDocBytes, &smallDoc)
	json.Unmarshal(tweetDocBytes, &tweetDoc)
	json.Unmarshal(largeDocBytes, &largeDoc)

	smallSize := len(smallDocBytes)
	tweetSize := len(tweetDocBytes)
	largeSize := len(largeDocBytes)

	ctx := context.Background()
	results := []BenchResult{}

	// Run Command (batch 10,000 hello commands per iteration)
	results = append(results, runBenchmark(
		"run_command", "single_doc",
		func(c *mongocore.Client) error { return nil },
		func(c *mongocore.Client) error { return nil },
		func(c *mongocore.Client) error {
			for i := 0; i < 10_000; i++ {
				if _, err := c.RunCommand(ctx, config.Database, bson.D{{Key: "hello", Value: 1}}, false); err != nil {
					return err
				}
			}
			return nil
		},
		func(c *mongocore.Client) error { return nil },
		func(c *mongocore.Client) error { return nil },
		10_000*100, 10_000, config,
	))

	// Find One by ID (batch 10,000 finds per iteration)
	results = append(results, runBenchmark(
		"find_one_by_id", "single_doc",
		func(c *mongocore.Client) error {
			doc := bson.D{{Key: "_id", Value: "bench_find_001"}}
			for k, v := range tweetDoc {
				if k != "_id" {
					doc = append(doc, bson.E{Key: k, Value: v})
				}
			}
			// Drop and recreate
			c.RunCommand(ctx, config.Database, bson.D{{Key: "drop", Value: "bench_find_mc"}}, false)
			_, err := c.Database(config.Database).Collection("bench_find_mc").InsertOne(ctx, doc)
			return err
		},
		func(c *mongocore.Client) error { return nil },
		func(c *mongocore.Client) error {
			for i := 0; i < 10_000; i++ {
				if _, err := c.Database(config.Database).Collection("bench_find_mc").FindOne(ctx, bson.D{{Key: "_id", Value: "bench_find_001"}}); err != nil {
					return err
				}
			}
			return nil
		},
		func(c *mongocore.Client) error { return nil },
		func(c *mongocore.Client) error {
			c.RunCommand(ctx, config.Database, bson.D{{Key: "drop", Value: "bench_find_mc"}}, false)
			return nil
		},
		10_000*tweetSize, 10_000, config,
	))

	// InsertOne Small (batch 10,000 inserts per iteration)
	results = append(results, runBenchmark(
		"insert_one_small", "single_doc",
		func(c *mongocore.Client) error { return nil },
		func(c *mongocore.Client) error {
			c.RunCommand(ctx, config.Database, bson.D{{Key: "drop", Value: "bench_insert_small_mc"}}, false)
			return nil
		},
		func(c *mongocore.Client) error {
			for i := 0; i < 10_000; i++ {
				doc := bson.D{{Key: "_id", Value: bson.NewObjectID().Hex()}}
				for k, v := range smallDoc {
					if k != "_id" {
						doc = append(doc, bson.E{Key: k, Value: v})
					}
				}
				if _, err := c.Database(config.Database).Collection("bench_insert_small_mc").InsertOne(ctx, doc); err != nil {
					return err
				}
			}
			return nil
		},
		func(c *mongocore.Client) error { return nil },
		func(c *mongocore.Client) error { return nil },
		10_000*smallSize, 10_000, config,
	))

	// InsertOne Large (batch 10 inserts per iteration, large docs ~2.75MB each)
	results = append(results, runBenchmark(
		"insert_one_large", "single_doc",
		func(c *mongocore.Client) error { return nil },
		func(c *mongocore.Client) error {
			c.RunCommand(ctx, config.Database, bson.D{{Key: "drop", Value: "bench_insert_large_mc"}}, false)
			return nil
		},
		func(c *mongocore.Client) error {
			for i := 0; i < 10; i++ {
				doc := bson.D{{Key: "_id", Value: bson.NewObjectID().Hex()}}
				for k, v := range largeDoc {
					if k != "_id" {
						doc = append(doc, bson.E{Key: k, Value: v})
					}
				}
				if _, err := c.Database(config.Database).Collection("bench_insert_large_mc").InsertOne(ctx, doc); err != nil {
					return err
				}
			}
			return nil
		},
		func(c *mongocore.Client) error { return nil },
		func(c *mongocore.Client) error { return nil },
		10*largeSize, 10, config,
	))

	// Bulk Insert Small (10K per iteration)
	results = append(results, runBenchmark(
		"bulk_insert_small", "multi_doc",
		func(c *mongocore.Client) error { return nil },
		func(c *mongocore.Client) error {
			c.RunCommand(ctx, config.Database, bson.D{{Key: "drop", Value: "bench_bulk_mc"}}, false)
			return nil
		},
		func(c *mongocore.Client) error {
			docs := make([]bson.D, 10_000)
			for i := 0; i < 10_000; i++ {
				doc := bson.D{{Key: "_id", Value: bson.NewObjectID().Hex()}}
				for k, v := range smallDoc {
					if k != "_id" {
						doc = append(doc, bson.E{Key: k, Value: v})
					}
				}
				docs[i] = doc
			}
			_, err := c.Database(config.Database).Collection("bench_bulk_mc").InsertMany(ctx, docs)
			return err
		},
		func(c *mongocore.Client) error { return nil },
		func(c *mongocore.Client) error { return nil },
		smallSize*10_000, 10_000, config,
	))

	// Find Many (10K docs — enabled with 64MB message limit)
	results = append(results, runBenchmark(
		"find_many", "multi_doc",
		func(c *mongocore.Client) error {
			c.RunCommand(ctx, config.Database, bson.D{{Key: "drop", Value: "bench_find_many_mc"}}, false)
			docs := make([]bson.D, 10_000)
			for i := 0; i < 10_000; i++ {
				doc := bson.D{{Key: "_id", Value: bson.NewObjectID().Hex()}}
				for k, v := range smallDoc {
					if k != "_id" {
						doc = append(doc, bson.E{Key: k, Value: v})
					}
				}
				docs[i] = doc
			}
			_, err := c.Database(config.Database).Collection("bench_find_many_mc").InsertMany(ctx, docs)
			return err
		},
		func(c *mongocore.Client) error { return nil },
		func(c *mongocore.Client) error {
			_, err := c.Database(config.Database).Collection("bench_find_many_mc").Find(ctx, bson.D{}, nil)
			return err
		},
		func(c *mongocore.Client) error { return nil },
		func(c *mongocore.Client) error { return nil },
		smallSize*10_000, 10_000, config,
	))

	// Bulk Insert Large (10 x 2.75MB docs — enabled with 64MB message limit)
	results = append(results, runBenchmark(
		"bulk_insert_large", "multi_doc",
		func(c *mongocore.Client) error { return nil },
		func(c *mongocore.Client) error {
			c.RunCommand(ctx, config.Database, bson.D{{Key: "drop", Value: "bench_bulk_large_mc"}}, false)
			return nil
		},
		func(c *mongocore.Client) error {
			docs := make([]bson.D, 10)
			for i := 0; i < 10; i++ {
				doc := bson.D{{Key: "_id", Value: bson.NewObjectID().Hex()}}
				for k, v := range largeDoc {
					if k != "_id" {
						doc = append(doc, bson.E{Key: k, Value: v})
					}
				}
				docs[i] = doc
			}
			_, err := c.Database(config.Database).Collection("bench_bulk_large_mc").InsertMany(ctx, docs)
			return err
		},
		func(c *mongocore.Client) error { return nil },
		func(c *mongocore.Client) error { return nil },
		largeSize*10, 10, config,
	))

	// Find Many Large (10 x 2.75MB docs — enabled with 64MB message limit)
	results = append(results, runBenchmark(
		"find_many_large", "multi_doc",
		func(c *mongocore.Client) error {
			c.RunCommand(ctx, config.Database, bson.D{{Key: "drop", Value: "bench_find_many_large_mc"}}, false)
			docs := make([]bson.D, 10)
			for i := 0; i < 10; i++ {
				doc := bson.D{{Key: "_id", Value: bson.NewObjectID().Hex()}}
				for k, v := range largeDoc {
					if k != "_id" {
						doc = append(doc, bson.E{Key: k, Value: v})
					}
				}
				docs[i] = doc
			}
			_, err := c.Database(config.Database).Collection("bench_find_many_large_mc").InsertMany(ctx, docs)
			return err
		},
		func(c *mongocore.Client) error { return nil },
		func(c *mongocore.Client) error {
			_, err := c.Database(config.Database).Collection("bench_find_many_large_mc").Find(ctx, bson.D{}, nil)
			return err
		},
		func(c *mongocore.Client) error { return nil },
		func(c *mongocore.Client) error { return nil },
		largeSize*10, 10, config,
	))

	// Save results
	resultsDir := filepath.Join("..", "..", "results")
	os.MkdirAll(resultsDir, 0755)
	outputPath := filepath.Join(resultsDir, "go_mongocore.json")
	resultsJSON, _ := json.MarshalIndent(results, "", "  ")
	if err := os.WriteFile(outputPath, resultsJSON, 0644); err != nil {
		panic(err)
	}
	fmt.Printf("\nResults saved to %s\n", outputPath)
}
