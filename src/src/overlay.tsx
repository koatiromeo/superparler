import React from 'react';
import ReactDOM from 'react-dom/client';
import { Hud } from './components/overlay/Hud';
import './styles.css';

ReactDOM.createRoot(document.getElementById('root')!).render(
  <React.StrictMode>
    <Hud />
  </React.StrictMode>,
);
