import { fork } from 'node:child_process';
import { openSync } from 'node:fs';

// console.log(process.argv);

if (process.argv[2] !== 'child') {
    console.log("I'm parent. spawning");
    fork(import.meta.filename, ['child']);
    console.log('exit');
    process.exit();
} else {
    console.log("child, reading files")
    setTimeout(() => {
        console.log('fd:', openSync('x'))
    }, 200);
}
