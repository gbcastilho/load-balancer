# Load Balancer ⚖️

Load balancer system for the discipline <b>Distributed Systems - UNICAMP</b></h3>

![GIF demo](img/demo.gif)

![Rust](https://img.shields.io/badge/rust-%23000000.svg?style=for-the-badge&logo=rust&logoColor=white)
![Docker](https://img.shields.io/badge/docker-%230db7ed.svg?style=for-the-badge&logo=docker&logoColor=white)

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

## Metrics
