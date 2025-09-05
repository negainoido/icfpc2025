import React from 'react';
import SolutionsList from '../components/SolutionsList';

const SolutionsPage: React.FC = () => {
  return (
    <div>
      <div style={{ 
        padding: '20px', 
        borderBottom: '1px solid #dee2e6', 
        backgroundColor: '#f8f9fa' 
      }}>
        <h1 style={{ margin: 0, color: '#333' }}>Solutions Dashboard</h1>
        <p style={{ margin: '8px 0 0 0', color: '#666' }}>
          View and manage problem solutions
        </p>
      </div>
      <SolutionsList />
    </div>
  );
};

export default SolutionsPage;