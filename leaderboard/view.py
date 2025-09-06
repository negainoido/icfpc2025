import json
from datetime import datetime
from pathlib import Path

import pandas as pd
import streamlit as st

RESULTS_DIR = Path("results")


def load_data() -> pd.DataFrame:
    """Load historical data from results directory"""
    json_files = sorted(RESULTS_DIR.glob("*.json"))
    if not json_files:
        return pd.DataFrame()

    all_data = []

    for json_file in json_files:
        try:
            with open(json_file, "r") as f:
                data = json.load(f)

            # Parse timestamp from filename
            timestamp_str = json_file.stem  # MMDD_HHMM
            month_day = timestamp_str[:4]
            hour_min = timestamp_str[5:]
            current_year = datetime.now().year
            dt = datetime.strptime(f"{current_year}{month_day}{hour_min}", "%Y%m%d%H%M")

            for team in data:
                team_name = team.get("teamName", "Unknown")
                score = team.get("score", 0)
                all_data.append(
                    {"timestamp": dt, "team_name": team_name, "score": score}
                )

        except (json.JSONDecodeError, ValueError, KeyError) as e:
            st.error(f"Error processing {json_file}: {e}")
            continue

    return pd.DataFrame(all_data)


def main():
    st.set_page_config(
        page_title="ICFPC 2025 Leaderboard", page_icon="📊", layout="wide"
    )

    st.title("ICFPC 2025 Leaderboard Progress")

    # Load data
    df = load_data()

    if df.empty:
        st.error("No data found in results directory")
        return

    # Team selection
    all_teams = sorted(df["team_name"].unique())

    # Default selection: top 10 teams by score + negainoido if not in top 10
    latest_timestamp = df["timestamp"].max()
    latest_scores = df[df["timestamp"] == latest_timestamp].sort_values(  # type: ignore
        "score", ascending=False
    )
    top_10_teams = latest_scores["team_name"].head(10).tolist()

    default_teams = top_10_teams
    if "negainoido" not in default_teams:
        default_teams.append("negainoido")

    selected_teams = st.multiselect(
        "Select teams to display", all_teams, default=default_teams
    )
    if not selected_teams:
        st.warning("Please select at least one team")
        return

    # Display filter data
    filtered_df = df[df["team_name"].isin(selected_teams)]
    chart_data = filtered_df.pivot(
        index="timestamp", columns="team_name", values="score"
    )
    st.line_chart(chart_data)

    # Show latest scores
    st.subheader("Latest Scores")
    latest_timestamp = df["timestamp"].max()
    latest_data = df[df["timestamp"] == latest_timestamp].sort_values(  # type: ignore
        "score", ascending=False
    )
    latest_data["rank"] = (
        latest_data["score"].rank(method="min", ascending=False).astype(int)
    )

    # Style negainoido team
    def highlight_negainoido(row):
        if row.iloc[1].lower() == "negainoido":  # Team column is at index 1
            return ["background-color: #ffcccc"] * len(row)
        return [""] * len(row)

    styled_data = latest_data[["rank", "team_name", "score"]].rename(  # type: ignore
        columns={"rank": "Rank", "team_name": "Team", "score": "Score"}
    )
    st.dataframe(
        styled_data.style.apply(highlight_negainoido, axis=1),
        use_container_width=True,
        hide_index=True,
        height=600,
    )


if __name__ == "__main__":
    main()
