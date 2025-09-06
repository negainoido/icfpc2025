# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

This is a Python-based leaderboard monitoring system for ICFPC 2025. The application fetches team rankings from the competition API and generates visualization graphs to track performance over time.

## Key Components

- `main.py`: Entry point script that fetches leaderboard data from AWS API endpoint
- `results/`: Directory for storing timestamped JSON data files (`MMDD_HHMM.json` format)
- Generated `result.png`: Output graph visualization

## Development Environment

- Python 3.12 (specified in `.python-version`)
- Uses `uv` for dependency management (presence of `uv.lock`)
- Virtual environment in `.venv/`

## Commands

### Setup and Installation
```bash
# Install dependencies
uv sync

# Activate virtual environment (if needed)
source .venv/bin/activate
```

### Running the Application
```bash
# Run the main leaderboard fetcher
python main.py

# Or with uv
uv run python main.py
```

## Application Behavior

The system is designed to:
1. Fetch data from `https://31pwr5t6ij.execute-api.eu-west-2.amazonaws.com/leaderboard/global`
2. Save results as timestamped JSON files in `results/MMDD_HHMM.json`
3. Run initially on startup, then every hour
4. Generate graphs showing:
   - X-axis: Time (MMDD_HHMM format)
   - Y-axis: Team scores
   - Focus on top 20 teams plus "negainoido" team specifically
5. Output final visualization as `result.png`

## Data Source

The API endpoint returns team scores in descending order. The application tracks score progression over time for competitive analysis.