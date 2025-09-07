import { BrowserRouter as Router, Routes, Route } from 'react-router-dom';
import './App.css';
import GamePage from './pages/GamePage';
import SessionsPage from './pages/SessionsPage';
import VisualizePage from './pages/VisualizePage';
import HomePage from './pages/HomePage';
import { SessionProvider } from './context/SessionContext';

function App() {
  return (
    <SessionProvider>
      <Router>
        <div className="App">
          <Routes>
            <Route path="/" element={<HomePage />} />
            <Route path="/game" element={<GamePage />} />
            <Route path="/sessions" element={<SessionsPage />} />
            <Route path="/visualize" element={<VisualizePage />} />
          </Routes>
        </div>
      </Router>
    </SessionProvider>
  );
}

export default App;
