import json
from datetime import datetime
from pathlib import Path

import pandas as pd
import streamlit as st

RESULTS_DIR = Path("results")
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


def load_data() -> pd.DataFrame:
    """Load historical data from results directory"""
    json_files = sorted((RESULTS_DIR / "global").glob("*.json"))
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


def load_problem_data() -> pd.DataFrame:
    """Load latest data for each problem"""
    problem_data = []

    for problem_name in PROBLEMS:
        problem_dir = RESULTS_DIR / problem_name
        if not problem_dir.exists():
            continue

        json_files = sorted(problem_dir.glob("*.json"))
        if not json_files:
            continue

        # Get latest file (last in sorted order)
        latest_file = json_files[-1]

        try:
            with open(latest_file, "r") as f:
                data = json.load(f)

            # Parse timestamp from filename
            timestamp_str = latest_file.stem  # MMDD_HHMM
            month_day = timestamp_str[:4]
            hour_min = timestamp_str[5:]
            current_year = datetime.now().year
            dt = datetime.strptime(f"{current_year}{month_day}{hour_min}", "%Y%m%d%H%M")

            # Sort teams by score (descending) and assign ranks
            teams_with_scores = []
            for team in data:
                team_name = team.get("teamName", "Unknown")
                score = team.get("score") or float("inf")
                teams_with_scores.append((team_name, score))

            teams_with_scores.sort(key=lambda x: x[1], reverse=False)

            # Find negainoido's rank and calculate Borda count
            negainoido_rank = None
            negainoido_score = None
            negainoido_borda = None

            for i, (team_name, score) in enumerate(teams_with_scores):
                if team_name == "negainoido":
                    negainoido_score = score
                    negainoido_rank = (
                        sum(1 for _, s in teams_with_scores if s < score) + 1
                    )
                    # Borda count = number of teams with strictly higher score (worse ranking)
                    negainoido_borda = sum(1 for _, s in teams_with_scores if s > score)
                    break

            problem_data.append(
                {
                    "problem": problem_name,
                    "timestamp": dt,
                    "rank": negainoido_rank if negainoido_rank else "Not found",
                    "score": negainoido_score if negainoido_score else 0,
                    "borda_count": negainoido_borda
                    if negainoido_borda is not None
                    else "Not found",
                    "total_teams": len(teams_with_scores),
                }
            )

        except (json.JSONDecodeError, ValueError, KeyError) as e:
            st.error(f"Error processing {latest_file}: {e}")
            continue

    return pd.DataFrame(problem_data)


def main():
    st.set_page_config(
        page_title="ICFPC 2025 Leaderboard", page_icon="📊", layout="wide"
    )

    st.title("ICFPC 2025 Leaderboard")

    # Load data
    df = load_data()

    if df.empty:
        st.error("No data found in results directory")
        return

    # Initialize session state for team selection
    # Check URL parameters for team selection
    query_params = st.query_params
    teams_param = query_params.get("teams", "")

    if "selected_teams" not in st.session_state:
        if teams_param:
            # Load teams from URL parameter
            selected_teams_from_url = set(teams_param.split(","))
            st.session_state.selected_teams = selected_teams_from_url
        else:
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

    tabs = st.tabs(["by Problems", "Global"])

    # by Problems
    with tabs[0]:
        st.subheader("negainoido's Rank by Problems")

        problem_df = load_problem_data()
        if problem_df.empty:
            st.error("No problem data found")
        else:
            # Calculate total Borda Count
            total_borda = 0
            for _, row in problem_df.iterrows():
                borda = row["borda_count"]
                if borda != "Not found" and borda is not None:
                    total_borda += borda

            # Display total Borda Count as a large metric
            st.metric(label="Total Borda Count", value=int(total_borda))

            st.write("")  # Add some spacing

            # Display problem rankings
            display_columns = ["problem", "rank", "score", "borda_count", "total_teams"]
            st.dataframe(
                problem_df[display_columns],
                column_config={
                    "problem": st.column_config.TextColumn("Problem"),
                    "rank": st.column_config.NumberColumn("Rank"),
                    "score": st.column_config.NumberColumn("Score"),
                    "borda_count": st.column_config.NumberColumn("Borda Count"),
                    "total_teams": st.column_config.NumberColumn("Total Teams"),
                },
                use_container_width=True,
                hide_index=True,
                height=600,
            )

    # Global
    with tabs[1]:
        st.subheader("Global Scores")
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

        # Update URL parameters with selected teams
        if selected_teams:
            teams_param = ",".join(selected_teams)
            st.query_params["teams"] = teams_param
        else:
            if "teams" in st.query_params:
                del st.query_params["teams"]

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
