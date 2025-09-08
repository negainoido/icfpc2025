import json
import time
from datetime import datetime, timedelta
from pathlib import Path
from typing import Any, Dict, List

import requests
import schedule

API_URL = "https://31pwr5t6ij.execute-api.eu-west-2.amazonaws.com/leaderboard"
RESULTS_DIR = Path("results")
RESULTS_DIR.mkdir(exist_ok=True)

PROBLEMS = [
    "primus",
    "secundus",
    "tertius",
    "quartus",
    "quintus",
    "aleph",
    "beth",
    "gimel",
    "daleth",
    "he",
    "vau",
    "zain",
    "hhet",
    "teth",
    "iod",
]


def fetch_leaderboard(problem_name: str = "global") -> List[Dict[str, Any]]:
    """Fetch leaderboard data"""
    try:
        response = requests.get(f"{API_URL}/{problem_name}", timeout=30)
        response.raise_for_status()
        return response.json()
    except requests.RequestException as e:
        print(f"Error fetching leaderboard: {e}")
        return []


def save_data(name: str, data: List[Dict[str, Any]], timestamp: str) -> None:
    """Save leaderboard data to timestamped JSON file"""
    if not data:
        return

    problem_dir = RESULTS_DIR / name
    problem_dir.mkdir(exist_ok=True)
    filename = problem_dir / f"{timestamp}.json"

    print("Saving data to", filename)
    with open(filename, "w") as f:
        json.dump(data, f, indent=2)


def fetch_and_save():
    """Fetch leaderboard data and save it"""
    timestamp = datetime.now().strftime("%m%d_%H%M")
    print(f"Fetching leaderboard data at {timestamp}")

    data = fetch_leaderboard()
    if data:
        save_data("global", data, timestamp)

    for problem_name in PROBLEMS:
        data = fetch_leaderboard(problem_name)
        if data:
            save_data(problem_name, data, timestamp)


def wait_for_next_10min_mark():
    """Wait until the next 10-minute mark (XX:00, XX:10, XX:20, etc.)"""
    now = datetime.now()
    current_minute = now.minute
    current_second = now.second

    # Calculate minutes to wait until next 10-minute mark
    minutes_to_wait = 10 - (current_minute % 10)
    if current_second == 0 and current_minute % 10 == 0:
        # Already at 10-minute mark
        return

    # Calculate total seconds to wait
    seconds_to_wait = (minutes_to_wait * 60) - current_second

    next_time = now.replace(second=0, microsecond=0) + timedelta(
        seconds=seconds_to_wait
    )
    print(f"Waiting until {next_time.strftime('%H:%M:%S')} for next 10-minute mark...")

    time.sleep(seconds_to_wait)


def main():
    print("Starting leaderboard monitor...")
    fetch_and_save()

    print("Scheduled to run every 10 minutes. Press Ctrl+C to stop.")
    wait_for_next_10min_mark()
    fetch_and_save()
    schedule.every(10).minutes.do(fetch_and_save)

    try:
        while True:
            schedule.run_pending()
            time.sleep(30)  # Check every 30 seconds
    except KeyboardInterrupt:
        print("\nStopping leaderboard monitor...")


if __name__ == "__main__":
    main()
