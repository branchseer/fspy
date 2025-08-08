#!/usr/bin/env node

import cp from 'node:child_process'
import fs from 'node:fs'
cp.execFileSync('cat', ['mise.toml'], { stdio: 'inherit'});
fs.readdirSync('/workspaces');
