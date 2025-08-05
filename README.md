
<!-- ABOUT THE PROJECT -->
## About The Project
An LLM (Gemini) API load balancer written in Rust/Axum.




<!-- USAGE EXAMPLES -->
## Usage

'''
services:
  llmkey_pool:
      image: ghcr.io/justinvulz/llmkey_pool
      ports:
        - "3030:3030"
      volumes:
        - "./config.toml:/app/config.toml"
'''






