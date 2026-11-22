import React from 'react';
import ReactDOM from 'react-dom/client';
// Fontes empacotadas localmente (sem CDN): a ferramenta funciona offline.
import '@fontsource/inter/400.css';
import '@fontsource/inter/600.css';
import '@fontsource/jetbrains-mono/400.css';
import '@fontsource/jetbrains-mono/600.css';
import './styles/global.css';
import './ui/glass/glass.css';
import { App } from './App';

ReactDOM.createRoot(document.getElementById('root') as HTMLElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
