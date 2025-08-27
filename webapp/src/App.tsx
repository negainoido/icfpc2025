import { BrowserRouter as Router, Routes, Route, Link } from 'react-router-dom'
import './App.css'
import SpaceshipPage from './pages/SpaceshipPage'

function App() {
  return (
    <Router>
      <div className="App">
        <nav style={{ padding: '20px', borderBottom: '1px solid #ccc' }}>
          <Link to="/" style={{ marginRight: '20px', textDecoration: 'none' }}>
            Home
          </Link>
          <Link to="/spaceship" style={{ textDecoration: 'none' }}>
            Spaceship Visualization
          </Link>
        </nav>
        
        <Routes>
          <Route path="/" element={<HomePage />} />
          <Route path="/spaceship" element={<SpaceshipPage />} />
          <Route path="/spaceship/:problemNumber" element={<SpaceshipPage />} />
        </Routes>
      </div>
    </Router>
  )
}

function HomePage() {
  return (
    <div style={{ padding: '20px', textAlign: 'center' }}>
      <h1>ICFPC 2025 Practice</h1>
      <p>React + TypeScript + Vite フロントエンド</p>
      <p>API Server: Rust + axum + MySQL</p>
      <div style={{ marginTop: '40px' }}>
        <Link 
          to="/spaceship" 
          style={{ 
            padding: '12px 24px', 
            background: '#007bff', 
            color: 'white', 
            textDecoration: 'none',
            borderRadius: '4px',
            display: 'inline-block'
          }}
        >
          View Spaceship Visualization
        </Link>
      </div>
    </div>
  )
}

export default App