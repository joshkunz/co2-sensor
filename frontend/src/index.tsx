import React from 'react';
import ReactDOM from 'react-dom';
import * as app from './app';

import 'bootstrap/dist/css/bootstrap.min.css';
import 'bootstrap-icons/font/bootstrap-icons.css'

import './index.css';

/* BEGIN MAGIC
 * 
 * This trick comes from the TypeScript handbook entry on optional imports.
 * TypeScript only emits a `require` for a module if it can detect that a
 * non-type declaration for that module is used. By guarding all our non-type
 * uses of the module behind an `if-statement` that tsc can prove is false
 * at build time, tsc will not include a require for the mocks. Then we can
 * manually add the require in the development environment.
 */
import { worker as W } from './mocks/browser'

declare function require(moduleName: string): any;

if (process.env.NODE_ENV === 'development') {
    let worker: typeof W = require('./mocks/browser').worker;
    worker.start();
}

/* END MAGIC */

ReactDOM.render(<app.App />, document.getElementById("root"));