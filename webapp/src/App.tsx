import { BrowserRouter as Router, Routes, Route, Link } from 'react-router-dom';
import './App.css';
import GamePage from './pages/GamePage';
import { SessionProvider } from './context/SessionContext';

function App() {
  return (
    <SessionProvider>
      <Router>
        <div className="App">
          <Routes>
            <Route path="/" element={<HomePage />} />
            <Route path="/game" element={<GamePage />} />
          </Routes>
        </div>
      </Router>
    </SessionProvider>
  );
}

function HomePage() {
  return (
    <div style={{ 
      minHeight: '100vh', 
      display: 'flex', 
      flexDirection: 'column', 
      justifyContent: 'center', 
      alignItems: 'center',
      backgroundColor: '#f8f9fa',
      padding: '20px',
      textAlign: 'center' 
    }}>
      <div style={{
        backgroundColor: 'white',
        borderRadius: '8px',
        padding: '40px',
        boxShadow: '0 4px 6px rgba(0, 0, 0, 0.1)',
        maxWidth: '600px',
      }}>
        <h1 style={{ marginBottom: '20px', color: '#343a40' }}>ICFPC 2025</h1>
        <h2 style={{ marginBottom: '30px', color: '#6c757d', fontWeight: 'normal' }}>
          Library Explorer
        </h2>
        
        <p style={{ marginBottom: '30px', color: '#6c757d', lineHeight: '1.6' }}>
          Explore mysterious libraries, map their layouts, and solve puzzles in this interactive challenge.
          Use the proxy API to safely interact with the ICFPC contest system.
        </p>

        <div style={{ marginBottom: '30px' }}>
          <Link
            to="/game"
            style={{
              padding: '15px 30px',
              backgroundColor: '#007bff',
              color: 'white',
              textDecoration: 'none',
              borderRadius: '6px',
              display: 'inline-block',
              fontSize: '18px',
              fontWeight: 'bold',
              transition: 'background-color 0.3s',
            }}
          >
            Start Exploring
          </Link>
        </div>

        <div style={{ fontSize: '14px', color: '#6c757d' }}>
          <p style={{ marginBottom: '10px' }}>
            <strong>Resources:</strong>
          </p>
          <p>
            <Link
              to="https://icfpcontest2025.github.io/"
              target="_blank"
              rel="noopener noreferrer"
              style={{ color: '#007bff', marginRight: '20px' }}
            >
              Official Contest Site
            </Link>
            <Link
              to="https://icfpcontest2025.github.io/specs/task_from_tex.html"
              target="_blank"
              rel="noopener noreferrer"
              style={{ color: '#007bff' }}
            >
              API Documentation
            </Link>
          </p>
        </div>
      </div>

      <div style={{
        marginTop: '30px',
        fontSize: '12px',
        color: '#adb5bd',
      }}>
        <p>Proxy API Server running on http://localhost:8080</p>
      </div>
    </div>
  );
}

export default App;
