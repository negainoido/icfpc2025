# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

This is a Python-based leaderboard monitoring system for ICFPC 2025 with two components: a data collection service and a web-based visualization dashboard.

## Architecture

### Data Collection (`main.py`)
- Fetches leaderboard data from AWS API endpoint every 10 minutes
- Saves timestamped JSON files in `results/MMDD_HHMM.json` format
- Uses `schedule` library for periodic execution
- Waits for 10-minute marks (XX:00, XX:10, XX:20, etc.) before starting

### Visualization (`view.py`)
- Streamlit-based web dashboard for interactive data visualization
- Loads historical data from JSON files in `results/` directory
- Displays latest scores in editable table with team selection
- Generates line charts showing score progression over time
- Supports URL parameters for sharing specific team selections
- Defaults to top 10 teams plus "negainoido" if not in top 10

## Commands

### Setup
```bash
# Install dependencies
uv sync
```

### Data Collection
```bash
# Run leaderboard data fetcher (runs every 10 minutes)
uv run python main.py
```

### Visualization
```bash
# Run Streamlit dashboard
uv run streamlit run view.py
```

### Code Quality
```bash
# Format code
uv run black .
uv run isort .

# Lint code
uv run ruff check .
```

## Data Format

JSON files in `results/` contain arrays of team objects:
```json
[
  {
    "teamName": "team_name",
    "score": 12345
  }
]
```

Filenames follow `MMDD_HHMM.json` format (e.g., `0906_1430.json`).
