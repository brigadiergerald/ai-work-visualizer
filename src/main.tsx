import React from 'react';
import {createRoot} from 'react-dom/client';
import FreeDashboard from './dashboard/FreeDashboard';
import {client} from './core/client';
const activeClient=import.meta.env.MODE==='open-core-fixture'?(await import('./core/offline-fixture')).fixtureClient:client;
createRoot(document.getElementById('root')!).render(<React.StrictMode><FreeDashboard client={activeClient}/></React.StrictMode>);
