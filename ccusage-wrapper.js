#!/usr/bin/env node

// This wrapper script allows us to call ccusage from the bundled node_modules
// It passes all arguments directly to ccusage

import { spawn } from 'child_process';
import { fileURLToPath } from 'url';
import { dirname, join } from 'path';

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);

// Find ccusage in node_modules
const ccusagePath = join(__dirname, 'node_modules', 'ccusage', 'dist', 'index.js');

// Spawn ccusage as a child process with all arguments
const child = spawn('node', [ccusagePath, ...process.argv.slice(2)], {
  stdio: 'inherit',
  env: process.env
});

child.on('exit', (code) => {
  process.exit(code || 0);
});

child.on('error', (err) => {
  console.error('Failed to run ccusage:', err);
  process.exit(1);
});