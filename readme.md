# KūKai

<img src="logo.png" alt="KūKai Mascot" width="150" align="right" />

![Rust](https://img.shields.io/badge/language-Rust-orange?style=flat-square)
![Apache Arrow](https://img.shields.io/badge/arrow-v44-blue?style=flat-square)
![Tonic](https://img.shields.io/badge/tonic-gRPC-blueviolet?style=flat-square)
![License](https://img.shields.io/badge/license-BSD%203--Clause-green?style=flat-square)

**KūKai** is a modular, _high-performance_ load-testing framework for TCP-based protocols.  
Inspired by the Hawaiian god **Kūkailimoku** (often called **Kū**), associated with warfare and strategic battles, **KūKai** aims to help you “wage war” on servers to test their capacity and resilience.

---

## Why "KūKai"?

- **Kū** (the Hawaiian war god) symbolizes **power and relentless force**, reflecting the nature of load testing.  
- **Kai** (“ocean”) conveys **unbounded scale** and **flood-like traffic**.  

Hence, **KūKai** suggests **unstoppable** traffic generation and **powerful** testing.

---

## Key Features

- **Three Operation Modes**  
  1. **Commander**: Orchestrates load tests, hosts a gRPC [Apache Arrow Flight](https://arrow.apache.org/docs/format/Flight.html) server to gather real-time telemetry.  
  2. **Edge**: Runs local load tests on worker nodes; connects to the Commander, streams metrics back for analysis.  
  3. **Standalone**: Simplest mode—runs a local load test, writes metrics to disk in Arrow format (no remote orchestration needed).

- **Real-Time Telemetry**  
  - **Edge → Commander** via Arrow Flight (gRPC).  
  - **Standalone** writes all metrics to an `.arrow` file for offline analysis.

- **Flexible TCP**  
  - Sends arbitrary payloads (e.g., HTTP, raw TCP).  
  - Tracks success/failure and latency per request.

- **Analytics-Ready**  
  - Arrow-based data for fast queries with Python’s **pyarrow**, Rust’s **DataFusion**, etc.

---

## Architecture Diagram

Below is a **Mermaid** chart that illustrates KūKai’s modes and data flow:

```mermaid
flowchart LR
    subgraph Commander Mode
        A[Commander<br>gRPC Arrow Flight Server] -- Orchestrates Tests --> B[Edge Nodes]
        B -- Telemetry --> A
    end

    subgraph Edge Mode
        B -- Local Load --> T(Target(s))
    end

    subgraph Standalone Mode
        C[Standalone<br>Local Load Test]
        C -- Writes Metrics --> F[kukai_metrics.arrow]
    end

    T((Servers<br>/ Targets))

    style A fill:#ffdddd,stroke:#ffaaaa,stroke-width:2px
    style B fill:#ddffdd,stroke:#aaffaa,stroke-width:2px
    style C fill:#dddfff,stroke:#aaaaff,stroke-width:2px
    style T fill:#fff2cc,stroke:#ffe599,stroke-width:2px
    style F fill:#ffe6cc,stroke:#ffd18e,stroke-width:2px
```

- **Commander** orchestrates test configurations, accumulates telemetry from edges.  
- **Edges** run local load tasks against targets, pushing metrics back to the commander.  
- **Standalone** runs everything locally, storing results in `.arrow`.

---

## Use Cases

1. **Multi-Region Load**  
   - Deploy edges across multiple data centers. A single commander collects metrics.  
2. **Microservices Stress**  
   - Evaluate how each service endpoint behaves under concurrency spikes.  
3. **Resilience Drills**  
   - Verify success rates, latencies, or error patterns under heavy load.  
4. **Simple Local Tests**  
   - Standalone mode is ideal for quick tests on a single machine.

---

## Configuration & Usage

All modes share a **TOML** config file. Key sections:

### `kukai_config.toml`

```toml
# "commander", "edge", or "standalone"
mode = "standalone"

[commander]
edges = ["127.0.0.1:50051"]

[edge]
commander_address = "127.0.0.1:50051"

[load]
rps = 50
duration_seconds = 10
concurrency = 2
payload = "GET / HTTP/1.1\r\nHost: example\r\n\r\n"
arrow_output = "kukai_metrics.arrow"

[[load.targets]]
addr = "127.0.0.1"
port = 8080
weight = 1.0

[[load.targets]]
addr = "127.0.0.1"
port = 9090
weight = 2.0
```

#### Fields:

- **mode**:
  - `commander`: Runs Arrow Flight server for telemetry.  
  - `edge`: Connects to the commander, runs the load test.  
  - `standalone`: Local testing, writes Arrow file to disk.
- **commander.edges**: List of edge node addresses (e.g. `["edge1:50051", "edge2:50051"]`)—used only if `mode=commander`.
- **edge.commander_address**: IP/Port for commander—only if `mode=edge`.
- **load**:
  - **rps**: Target requests per second.  
  - **duration_seconds**: How long to run the test.  
  - **concurrency**: Number of parallel worker tasks.  
  - **payload**: The data to send over TCP.  
  - **arrow_output**: Path to `.arrow` file (used in standalone mode).  
  - **targets**: One or more `{addr, port, weight}` blocks, for random/weighted selection.

---

## Running KūKai

1. **Build/Install**

   ```bash
   git clone https://github.com/copyleftdev/kukai.git
   cd kukai
   cargo build --release
   ```

2. **Choose a Mode**

   - **Standalone** (simple local test):
     ```bash
     # In kukai_config.toml: mode = "standalone"
     cargo run --release -- --config kukai_config.toml
     ```
     - Generates `kukai_metrics.arrow` locally.
   
   - **Commander**:
     ```bash
     # Terminal A
     # In kukai_config.toml: mode = "commander"
     cargo run --release -- --config kukai_config.toml
     ```
     - Waits on `0.0.0.0:50051` for edges to connect.
   
   - **Edge**:
     ```bash
     # Terminal B
     # In kukai_config.toml: mode = "edge"
     # (pointing commander_address to the Commander)
     cargo run --release -- --config kukai_config.toml
     ```
     - Spawns local workers, sends metrics via Arrow Flight.

---

## Analyzing Results

- **Standalone**  
  - After the test, an Arrow file (`.arrow`) is created.  
  - Inspect with Python:
    ```python
    import pyarrow as pa
    import pyarrow.ipc as ipc

    with pa.memory_map('kukai_metrics.arrow', 'r') as f:
        reader = ipc.RecordBatchFileReader(f)
        table = reader.read_all()
        df = table.to_pandas()
        print(df.head())
    ```

- **Commander**  
  - By default, the commander accumulates raw Arrow Flight chunks in memory (in the reference skeleton).
  - Extend it to decode or write them to disk as `.arrow`.

---

## Future Plans

- **Stricter RPS Enforcement** – Integrate a token-bucket or [governor](https://crates.io/crates/governor) for precise rate limiting.  
- **Live Orchestration** – Commander can dynamically adjust concurrency or payload on edges.  
- **Authentication / TLS** – Secure gRPC channels for production.  
- **Persistent Storage** – Automatic writing of commander-collected data to `.arrow` or a big-data pipeline.

---

## Contributing

1. Fork & clone: [KūKai on GitHub](https://github.com/copyleftdev/kukai).  
2. Create a feature branch, commit your changes, then open a Pull Request.  
3. Submit bug reports or enhancements via GitHub issues.

---

## License

**BSD 3-Clause** © 2025 [CopyleftDev](https://github.com/copyleftdev)

```
Redistribution and use in source and binary forms, with or without modification, are permitted provided
that the following conditions are met:

1. Redistributions of source code must retain the above copyright notice, this list of conditions and 
   the following disclaimer.
2. Redistributions in binary form must reproduce the above copyright notice, this list of conditions 
   and the following disclaimer in the documentation and/or other materials provided with the distribution.
3. Neither the name of CopyleftDev nor the names of its contributors may be used to endorse or promote 
   products derived from this software without specific prior written permission.

THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS “AS IS” AND ANY EXPRESS OR IMPLIED
WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR 
A PARTICULAR PURPOSE ARE DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT HOLDER OR CONTRIBUTORS BE LIABLE 
FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT 
LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS 
INTERRUPTION) HOWEVER CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY, OR 
TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE OF THIS SOFTWARE, EVEN IF 
ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
```

**Mahalo nui!**  
KūKai is built for the community—happy load testing!
