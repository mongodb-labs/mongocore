function dashboard() {
    return {
        window: '5m',
        opsChart: null,
        latencyChart: null,
        pollInterval: null,

        init() {
            this.$nextTick(() => {
                this.initCharts();
                this.fetchMetrics();
                this.pollInterval = setInterval(() => this.fetchMetrics(), 2000);
            });
        },

        destroy() {
            if (this.pollInterval) clearInterval(this.pollInterval);
            if (this.opsChart) this.opsChart.destroy();
            if (this.latencyChart) this.latencyChart.destroy();
        },

        setWindow(w) {
            this.window = w;
            this.fetchMetrics();
        },

        initCharts() {
            const opsEl = document.getElementById('chart-ops');
            const latEl = document.getElementById('chart-latency');
            if (!opsEl || !latEl) return;

            const baseOpts = {
                width: opsEl.clientWidth || 400,
                height: 200,
                cursor: { show: true },
                scales: { x: { time: false } },
            };

            this.opsChart = new uPlot({
                ...baseOpts,
                series: [
                    { label: 'Time' },
                    { label: 'ops/s', stroke: '#22c55e', width: 2 }
                ],
                axes: [
                    { show: true },
                    { label: 'ops/s' }
                ]
            }, [[], []], opsEl);

            this.latencyChart = new uPlot({
                ...baseOpts,
                width: latEl.clientWidth || 400,
                series: [
                    { label: 'Time' },
                    { label: 'p50', stroke: '#3b82f6', width: 2 },
                    { label: 'p95', stroke: '#f59e0b', width: 2 },
                    { label: 'p99', stroke: '#ef4444', width: 2 }
                ],
                axes: [
                    { show: true },
                    { label: 'ms' }
                ]
            }, [[], [], [], []], latEl);
        },

        async fetchMetrics() {
            try {
                const resp = await fetch('/api/metrics?window=' + this.window);
                if (!resp.ok) return;
                const data = await resp.json();

                if (this.opsChart && data.timestamps && data.ops_per_sec) {
                    this.opsChart.setData([data.timestamps, data.ops_per_sec]);
                }

                if (this.latencyChart && data.timestamps && data.p50) {
                    this.latencyChart.setData([data.timestamps, data.p50, data.p95, data.p99]);
                }
            } catch (e) {
                // Silently ignore fetch errors
            }
        }
    };
}
