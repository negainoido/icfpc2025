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

    # Initialize session state for team selection
    if "selected_teams" not in st.session_state:
        # Default selection: top 10 teams by score + negainoido if not in top 10
        latest_timestamp = df["timestamp"].max()
        latest_scores = df[df["timestamp"] == latest_timestamp].sort_values(  # type: ignore
            "score", ascending=False
        )
        top_10_teams = latest_scores["team_name"].head(10).tolist()

        default_teams = set(top_10_teams)
        if "negainoido" not in default_teams:
            default_teams.add("negainoido")

        st.session_state.selected_teams = default_teams

    # Show latest scores with checkboxes
    st.subheader("Latest Scores")
    latest_timestamp = df["timestamp"].max()
    latest_data = df[df["timestamp"] == latest_timestamp].sort_values(  # type: ignore
        "score", ascending=False
    )
    latest_data["rank"] = (
        latest_data["score"].rank(method="min", ascending=False).astype(int)
    )

    # Create display data with checkboxes
    display_data = []
    for _, row in latest_data.iterrows():
        team_name = row["team_name"]
        checked = team_name in st.session_state.selected_teams
        display_data.append(
            {
                "Chart": checked,
                "Rank": int(row["rank"]),
                "Team": team_name,
                "Score": int(row["score"]),
            }
        )

    display_df = pd.DataFrame(display_data)

    # Display editable dataframe
    edited_df = st.data_editor(
        display_df,
        column_config={
            "Chart": st.column_config.CheckboxColumn(
                "Chart",
                help="Select teams to display in chart",
                default=False,
            ),
            "Rank": st.column_config.NumberColumn("Rank", disabled=True),
            "Team": st.column_config.TextColumn("Team", disabled=True),
            "Score": st.column_config.NumberColumn("Score", disabled=True),
        },
        disabled=["Rank", "Team", "Score"],
        use_container_width=True,
        hide_index=True,
        height=600,
    )

    # Display chart based on selected teams from the edited dataframe
    selected_teams = []
    for _, row in edited_df.iterrows():
        if row["Chart"]:
            selected_teams.append(row["Team"])

    if selected_teams:
        filtered_df = df[df["team_name"].isin(selected_teams)]
        # Remove duplicates by keeping the last entry for each timestamp-team combination
        filtered_df = filtered_df.drop_duplicates(
            subset=["timestamp", "team_name"], keep="last"
        )
        chart_data = filtered_df.pivot(
            index="timestamp", columns="team_name", values="score"
        )
        st.line_chart(chart_data)
    else:
        st.warning("Please select at least one team to display in the chart")


if __name__ == "__main__":
    main()
