# Chat server

## Running locally

Cwd to the root folder (usually where this readme is located) and run `./cargo run client` or `./cargo run server`. See `./cargo run` help for additional options.

## Running with Prometheus and Grafana for metrics
1. Install Docker (easiest via [Docker Desktop](https://www.docker.com/products/docker-desktop/))
2. Cwd to the project root directory (where this file is located)
3. Run `docker-compose up` (image building can take few minutes for the first time)
4. Chat admin console is available at http://localhost:8080
5. Prometheus console is available at http://localhost:9090
6. Grafana is available at http://localhost:3000

### Collected metrics:
Metrics are collected for the duration of the server running. They are reset each time the server is stopped. Next instance will start counting from the beginning (usually 0).

Following metrics are collected
- Total number of messages sent through the server
- Number of connected users (via the text console) at that particular moment in time
- Number of ms the SQL queries took to execute (histogram)
