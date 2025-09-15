<h1 align="center">Load Balancer ⚖️</h1>

<p align="center">Load balancer system for the discipline <b>Distributed Systems - UNICAMP</b></p>

<p align="center">
<img src="https://img.shields.io/badge/rust-%23000000.svg?style=for-the-badge&logo=rust&logoColor=white">
<img src="https://img.shields.io/badge/docker-%230db7ed.svg?style=for-the-badge&logo=docker&logoColor=white">
  <a href="https://opensource.org/licenses/MIT">
    <img src="https://img.shields.io/badge/License-MIT-yellow.svg">
  </a>
</p>

![GIF demo](img/demo.gif)

A load balancer simulator handles arriving requests and distributes them between three independent servers.

## Usage
To run the project you have two options:

  1. Docker (Recommended)

If you have docker, you can build and run the project with simple commands


```bash
docker compose build
```
```bash
docker compose run --rm load-balancer
```

  2. Manual

If you prefer you can install [rust](https://www.rust-lang.org/) mannualy and run through [cargo](https://crates.io/)

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```
```bash
cargo run
```

## Options
It is possible to define how the system behaves

### Balancing Mode
- **Random**: The servers are chosen randomly.
- **Round Robin**: The servers are chosen uniformly, regardless of their workload.
- **Smaller Queue**: The server with the smallest request queue (i.e. the smallest workload) is chosen.

### Arrival Rate (λ)
You can set the average number of requests arriving per second between 0 and 10.

## Requests
Requests are defined by type and size.

- **Type**
  - **CPU Bound**: It demands more CPU computing.
  - **IO Bound**: It demands more input/output waiting time.
  - **Mixed**: A mixture of previous types.

- **Size**
  - **Small**: `100ms`
  - **Mid**: `300ms`
  - **Large**: `1000ms`

## Metrics
- **Total Requests**: The total number of requests received.
- **Processed**:  Number of successfully processed requests.
- **Avarage Response Time**: The average time taken to process a request since its arrival.
- **Throughput**: Number of requests processed per second.

## Capacity
Currently, the maximum number of requests that each queue can store is hard-coded. However, you can easily modify this to test new scenarios.
- **Server (each)**: 10 requests
- **Pending list**: 20 requests
