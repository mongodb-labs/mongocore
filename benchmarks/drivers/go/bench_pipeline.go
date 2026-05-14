// Benchmark MongoCore Go pipeline batching at different batch sizes.
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
	pb "github.com/rozza/mongocore/clients/go/proto"
	"go.mongodb.org/mongo-driver/v2/bson"
)

type Config struct {
	MongoDBURI    string         `json:"mongodb_uri"`
	MongoCoreAddr string         `json:"mongocore_address"`
	Database      string         `json:"database"`
	MinTimeSecs   int            `json:"min_time_secs"`
	MaxIterations int            `json:"max_iterations"`
	MaxTimeSecs   int            `json:"max_time_secs"`
	WarmupIters   map[string]int `json:"warmup_iterations"`
}

type SystemInfo struct {
	OS           string `json:"os"`
	Arch         string `json:"arch"`
	CPUs         int    `json:"cpus"`
	MongoCoreVer string `json:"mongocore_version"`
	Driver       string `json:"driver"`
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
	Benchmark     string      `json:"benchmark"`
	Category      string      `json:"category"`
	Driver        string      `json:"driver"`
	DatasetBytes  int         `json:"dataset_size_bytes"`
	BatchSize     int         `json:"batch_size"`
	Iterations    int         `json:"iterations"`
	TotalTimeSecs float64     `json:"total_time_secs"`
	OpsPerSec     float64     `json:"ops_per_sec"`
	MBPerSec      float64     `json:"mb_per_sec"`
	Percentiles   Percentiles `json:"percentiles"`
	Timestamp     string      `json:"timestamp"`
	System        SystemInfo  `json:"system"`
}

var batchSizes = []int{100, 1000, 10000}

const totalOps = 10000

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
	setupFn func() error,
	beforeTaskFn func() error,
	taskFn func() error,
	afterTaskFn func() error,
	teardownFn func() error,
	datasetBytes, batchSize int,
	config Config,
) BenchResult {
	if err := setupFn(); err != nil {
		panic(err)
	}

	warmup := config.WarmupIters["go"]
	for i := 0; i < warmup; i++ {
		_ = beforeTaskFn()
		_ = taskFn()
		_ = afterTaskFn()
	}

	times := []float64{}
	totalTime := 0.0
	iteration := 0

	for totalTime < float64(config.MinTimeSecs) || iteration < 5 {
		if iteration >= config.MaxIterations || totalTime >= float64(config.MaxTimeSecs) {
			break
		}

		_ = beforeTaskFn()
		start := time.Now()
		if err := taskFn(); err != nil {
			panic(err)
		}
		elapsed := time.Since(start).Seconds()
		_ = afterTaskFn()

		times = append(times, elapsed)
		totalTime += elapsed
		iteration++
	}

	_ = teardownFn()

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

	fmt.Printf("  %s: %.0f ops/s, %.2f MB/s (%d iterations)\n",
		name, opsPerSec, mbPerSec, len(times))
	return result
}

func encodeBson(doc interface{}) []byte {
	data, err := bson.Marshal(doc)
	if err != nil {
		panic(err)
	}
	return data
}

func main() {
	fmt.Println("=== MongoCore+Go Pipeline benchmarks ===")

	configPath := filepath.Join("..", "common.json")
	configData, err := os.ReadFile(configPath)
	if err != nil {
		panic(err)
	}
	var config Config
	json.Unmarshal(configData, &config)

	dataDir := filepath.Join("..", "..", "data")
	smallDocData, _ := os.ReadFile(filepath.Join(dataDir, "small_doc.json"))
	tweetDocData, _ := os.ReadFile(filepath.Join(dataDir, "tweet.json"))

	var smallDoc map[string]interface{}
	var tweetDoc map[string]interface{}
	json.Unmarshal(smallDocData, &smallDoc)
	json.Unmarshal(tweetDocData, &tweetDoc)

	smallSize := len(smallDocData)
	tweetSize := len(tweetDocData)

	client := mongocore.MongoClientTCP(config.MongoCoreAddr)
	ctx := context.Background()
	if err := client.Connect(ctx); err != nil {
		panic(err)
	}
	defer client.Close()

	results := []BenchResult{}

	for _, bs := range batchSizes {
		callsPerIter := totalOps / bs

		// --- pipeline_run_command ---
		results = append(results, runBenchmark(
			fmt.Sprintf("pipeline_run_command_%d", bs), "pipeline",
			func() error { return nil },
			func() error { return nil },
			func() error {
				for c := 0; c < callsPerIter; c++ {
					ops := make([]*pb.PipelineOperation, bs)
					for i := range ops {
						ops[i] = &pb.PipelineOperation{
							Operation: &pb.PipelineOperation_RunCommand{
								RunCommand: &pb.RunCommandRequest{
									Database: config.Database,
									Command:  &pb.Document{Data: encodeBson(bson.D{{"hello", 1}})},
								},
							},
						}
					}
					_, err := client.Pipeline(ctx, ops...)
					if err != nil {
						return err
					}
				}
				return nil
			},
			func() error { return nil },
			func() error { return nil },
			totalOps*100, totalOps, config,
		))

		// --- pipeline_insert_one_small ---
		collInsert := "bench_pipeline_insert_go"
		results = append(results, runBenchmark(
			fmt.Sprintf("pipeline_insert_one_small_%d", bs), "pipeline",
			func() error { return nil },
			func() error {
				client.RunCommand(ctx, config.Database, bson.D{{"drop", collInsert}}, false)
				return nil
			},
			func() error {
				for c := 0; c < callsPerIter; c++ {
					ops := make([]*pb.PipelineOperation, bs)
					for i := range ops {
						doc := bson.D{{"_id", bson.NewObjectID().Hex()}}
						for k, v := range smallDoc {
							doc = append(doc, bson.E{Key: k, Value: v})
						}
						ops[i] = &pb.PipelineOperation{
							Operation: &pb.PipelineOperation_Insert{
								Insert: &pb.InsertRequest{
									Database:   config.Database,
									Collection: collInsert,
									Document:   &pb.Document{Data: encodeBson(doc)},
								},
							},
						}
					}
					_, err := client.Pipeline(ctx, ops...)
					if err != nil {
						return err
					}
				}
				return nil
			},
			func() error { return nil },
			func() error { return nil },
			totalOps*smallSize, totalOps, config,
		))

		// --- pipeline_find_one_by_id ---
		collFind := "bench_pipeline_find_go"
		results = append(results, runBenchmark(
			fmt.Sprintf("pipeline_find_one_by_id_%d", bs), "pipeline",
			func() error {
				client.RunCommand(ctx, config.Database, bson.D{{"drop", collFind}}, false)
				doc := bson.D{{"_id", "bench_find_001"}}
				for k, v := range tweetDoc {
					doc = append(doc, bson.E{Key: k, Value: v})
				}
				coll := client.Database(config.Database).Collection(collFind)
				_, err := coll.InsertOne(ctx, doc)
				return err
			},
			func() error { return nil },
			func() error {
				for c := 0; c < callsPerIter; c++ {
					ops := make([]*pb.PipelineOperation, bs)
					for i := range ops {
						ops[i] = &pb.PipelineOperation{
							Operation: &pb.PipelineOperation_FindOne{
								FindOne: &pb.FindOneRequest{
									Database:   config.Database,
									Collection: collFind,
									Filter:     &pb.Filter{Data: encodeBson(bson.D{{"_id", "bench_find_001"}})},
								},
							},
						}
					}
					_, err := client.Pipeline(ctx, ops...)
					if err != nil {
						return err
					}
				}
				return nil
			},
			func() error { return nil },
			func() error {
				client.RunCommand(ctx, config.Database, bson.D{{"drop", collFind}}, false)
				return nil
			},
			totalOps*tweetSize, totalOps, config,
		))
	}

	resultsDir := filepath.Join("..", "..", "results")
	os.MkdirAll(resultsDir, 0755)
	outputPath := filepath.Join(resultsDir, "go_pipeline.json")
	data, _ := json.MarshalIndent(results, "", "  ")
	os.WriteFile(outputPath, data, 0644)
	fmt.Printf("\nResults saved to %s\n", outputPath)
}
