import { Link } from 'react-router-dom';
import './HomePage.css';

function HomePage() {
  return (
    <div className="home-container">
      <div className="home-box">
        <h1 className="home-title">ICFPC 2025</h1>
        <h2 className="home-subtitle">Library Explorer</h2>

        <p className="home-description">
          Explore mysterious libraries, map their layouts, and solve puzzles in
          this interactive challenge. Use the proxy API to safely interact with
          the ICFPC contest system.
        </p>

        <div className="home-buttons">
          <Link to="/game" className="home-button explore-btn">
            Start Exploring
          </Link>
          <Link to="/sessions" className="home-button sessions-btn">
            View Sessions
          </Link>
          <Link to="/visualize" className="home-button visualize-btn">
            Visualize Map
          </Link>
        </div>

        <div className="resource-section">
          <p className="resource-title">
            <strong>Resources:</strong>
          </p>
          <p>
            <Link
              to="https://icfpcontest2025.github.io/"
              target="_blank"
              rel="noopener noreferrer"
              className="resource-link"
            >
              Official Contest Site
            </Link>
            <Link
              to="https://icfpcontest2025.github.io/specs/task_from_tex.html"
              target="_blank"
              rel="noopener noreferrer"
              className="resource-link"
            >
              API Documentation
            </Link>
          </p>
        </div>
      </div>

      <div className="home-footer">
        <p>Proxy API Server running on http://localhost:8080</p>
      </div>
    </div>
  );
}

export default HomePage;
