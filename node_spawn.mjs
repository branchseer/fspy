import cp from 'node:child_process'
import fs from 'node:fs'

cp.execFileSync('/bin/cat', ['mise.toml'], { stdio: 'inherit'});

fs.readFileSync('mise.toml233');


