import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';
export default defineConfig({plugins:[react()], publicDir:false, clearScreen:false, server:{host:'127.0.0.1',port:1420,strictPort:true},envPrefix:['VITE_','TAURI_'],build:{target:'chrome110',outDir:'dist-open-core'}});
