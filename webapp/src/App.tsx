import { BrowserRouter as Router, Routes, Route, Link } from 'react-router-dom';
import './App.css';
import SpaceshipPage from './pages/SpaceshipPage';
import SolutionsPage from './pages/SolutionsPage';

function App() {
  return (
    <Router>
      <div className="App">
        <nav style={{ padding: '20px', borderBottom: '1px solid #ccc' }}>
          <Link to="/" style={{ marginRight: '20px', textDecoration: 'none' }}>
            Home
          </Link>
          <Link
            to="/spaceship"
            style={{ marginRight: '20px', textDecoration: 'none' }}
          >
            Spaceship Visualization
          </Link>
          <Link to="/solutions" style={{ textDecoration: 'none' }}>
            Solutions
          </Link>
        </nav>

        <Routes>
          <Route path="/" element={<HomePage />} />
          <Route path="/spaceship" element={<SpaceshipPage />} />
          <Route path="/spaceship/:problemNumber" element={<SpaceshipPage />} />
          <Route path="/solutions" element={<SolutionsPage />} />
        </Routes>
      </div>
    </Router>
  );
}

function HomePage() {
  return (
    <div style={{ padding: '20px', textAlign: 'center' }}>
      <h1>ICFPC 2025</h1>
      <p>
        <Link
          to="https://docs.google.com/document/d/1w9wMQRWhvEfrRe4MAXE_PDV5g14uwDLjSWmGnoTZUZI/edit?tab=t.0#heading=h.3lew8gtggl01"
          target="_blank"
        >
          Docs
        </Link>
      </p>
      <p>
        <Link to="https://icfpcontest2025.github.io/" target="_blank">
          Official Page
        </Link>
      </p>
      <div style={{ marginTop: '40px' }}>
        <Link
          to="/spaceship"
          style={{
            padding: '12px 24px',
            background: '#007bff',
            color: 'white',
            textDecoration: 'none',
            borderRadius: '4px',
            display: 'inline-block',
            marginRight: '20px',
          }}
        >
          View Spaceship Visualization
        </Link>
        <Link
          to="/solutions"
          style={{
            padding: '12px 24px',
            background: '#28a745',
            color: 'white',
            textDecoration: 'none',
            borderRadius: '4px',
            display: 'inline-block',
          }}
        >
          View Solutions
        </Link>
      </div>
    </div>
  );
}

export default App;
